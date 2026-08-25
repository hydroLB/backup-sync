import { Component, ErrorInfo, Fragment, ReactNode } from 'react';

type Props = {
  children: ReactNode;
  onReload?: () => void;
};

type State = {
  failed: boolean;
  recoveryAttempt: number;
};

/** Keeps an unexpected UI failure contained and gives the user a clean recovery path. */
export class AppErrorBoundary extends Component<Props, State> {
  state: State = {
    failed: false,
    recoveryAttempt: 0,
  };

  static getDerivedStateFromError(): Partial<State> {
    return { failed: true };
  }

  componentDidCatch(error: Error, info: ErrorInfo): void {
    if (import.meta.env.DEV) {
      console.error('The application UI stopped unexpectedly.', error, info.componentStack);
    }
  }

  private retry = (): void => {
    this.setState(({ recoveryAttempt }) => ({
      failed: false,
      recoveryAttempt: recoveryAttempt + 1,
    }));
  };

  render(): ReactNode {
    if (this.state.failed) {
      return (
        <main className="app" aria-labelledby="app-recovery-title">
          <section
            className="state-block state-error"
            role="alert"
            aria-labelledby="app-recovery-title"
          >
            <div className="state-block__content">
              <div
                className="state-block__title"
                id="app-recovery-title"
                role="heading"
                aria-level={1}
              >
                Backup Sync needs a moment
              </div>
              <div className="state-block__message">
                Something unexpected interrupted the screen. Reopen the app and check backup status
                before repeating an operation. If the problem continues, review the application log.
              </div>
            </div>
            <div className="state-block__action">
              <button className="btn" type="button" onClick={this.retry}>
                Retry
              </button>
              {this.props.onReload && (
                <button className="btn secondary" type="button" onClick={this.props.onReload}>
                  Reload app
                </button>
              )}
            </div>
          </section>
        </main>
      );
    }

    return <Fragment key={this.state.recoveryAttempt}>{this.props.children}</Fragment>;
  }
}
