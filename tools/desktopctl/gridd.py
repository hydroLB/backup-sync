#!/usr/bin/env python3

from __future__ import annotations

import argparse
import json

from grid_orchestrator import build_http_server


def build_parser() -> argparse.ArgumentParser:
    """
    Summary
    Construct the CLI parser for the grid orchestrator daemon.

    Inputs
    None.

    Outputs
    A configured `ArgumentParser`.

    Side effects
    None.

    Error handling
    None.

    Ties to other methods
    Used by `main`.

    Why this exists
    The orchestrator should be configurable without editing source code.
    """

    parser = argparse.ArgumentParser(
        prog="gridd",
        description="Concurrent mixed-worker test grid orchestrator for browser and GUI automation jobs.",
    )
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=8766)
    parser.add_argument("--desktop-base-url", default="http://127.0.0.1:8765")
    return parser


def main() -> int:
    """
    Summary
    Start the grid orchestrator daemon and serve until interrupted.

    Inputs
    Process arguments from `sys.argv`.

    Outputs
    Process exit status.

    Side effects
    Starts the orchestrator scheduler and an HTTP server.

    Error handling
    Prints contextual startup metadata and exits cleanly on keyboard interrupt.

    Ties to other methods
    Calls `build_parser` and `build_http_server`.

    Why this exists
    The orchestrator needs a stable standalone process for agents and tools to submit jobs to.
    """

    parser = build_parser()
    args = parser.parse_args()
    server, orchestrator = build_http_server(
        host=args.host,
        port=args.port,
        desktop_base_url=args.desktop_base_url,
    )
    orchestrator.start()
    print(
        json.dumps(
            {
                "host": args.host,
                "port": args.port,
                "jobs_url": f"http://{args.host}:{args.port}/jobs",
                "workers_url": f"http://{args.host}:{args.port}/workers",
                "health_url": f"http://{args.host}:{args.port}/health",
                "desktop_base_url": args.desktop_base_url,
            }
        ),
        flush=True,
    )
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()
        orchestrator.stop()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
