// Application configuration
// This file defines the base path for routing and asset loading

/**
 * Get the base path for the application
 * Priority: Environment variable > Default from config
 */
export const getBasePath = (): string => {
  // Check for environment variable override (used in CI)
  const envBasePath = import.meta.env.VITE_BASE_PATH;
  if (envBasePath) {
    return envBasePath;
  }

  // Default configuration based on environment
  const isDevelopment = import.meta.env.DEV;
  
  if (isDevelopment) {
    return '/';
  }

  // Default production base path - can be overridden via VITE_BASE_PATH env var
  return '/silly-demos';
};

/**
 * Get the base path with trailing slash for Vite config
 */
export const getViteBasePath = (): string => {
  const basePath = getBasePath();
  return basePath === '/' ? '/' : `${basePath}/`;
};

/**
 * Get the base path without trailing slash for React Router
 */
export const getRouterBasePath = (): string => {
  const basePath = getBasePath();
  return basePath === '/' ? '/' : basePath;
};
