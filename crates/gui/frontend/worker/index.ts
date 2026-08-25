type Env = {
  ASSETS: Fetcher;
};

const SECURITY_HEADERS: Readonly<Record<string, string>> = {
  'Content-Security-Policy':
    "default-src 'self'; script-src 'self'; style-src 'self'; connect-src 'self'; img-src 'self' data: blob:; font-src 'self' data:; object-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'; worker-src 'self' blob:",
  'Cross-Origin-Opener-Policy': 'same-origin',
  'Cross-Origin-Resource-Policy': 'same-origin',
  'Permissions-Policy':
    'camera=(), microphone=(), geolocation=(), payment=(), usb=(), serial=(), bluetooth=()',
  'Referrer-Policy': 'no-referrer',
  'Strict-Transport-Security': 'max-age=31536000; includeSubDomains',
  'X-Content-Type-Options': 'nosniff',
  'X-Frame-Options': 'DENY',
};

/** Serve the shared app shell while enforcing headers independent of hosting defaults. */
export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const response = await env.ASSETS.fetch(request);
    const secured = new Response(response.body, response);
    for (const [name, value] of Object.entries(SECURITY_HEADERS)) {
      secured.headers.set(name, value);
    }

    const path = new URL(request.url).pathname;
    if (/^\/assets\/[^/]+\.[A-Za-z0-9]+$/.test(path)) {
      secured.headers.set('Cache-Control', 'public, max-age=31536000, immutable');
    } else if (path === '/' || path.endsWith('.html')) {
      secured.headers.set('Cache-Control', 'no-cache');
    }
    return secured;
  },
};
