/** Build-time selection prevents a broken native bridge from silently becoming the web edition. */
export const IS_WEB_RUNTIME = import.meta.env.VITE_APP_RUNTIME === 'web';
