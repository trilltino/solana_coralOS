import type { NextConfig } from 'next'
import path from 'path'

const nextConfig: NextConfig = {
  // Allow Next.js to transpile the TypeScript SDK source directly.
  // This resolves .js extensions to .ts when importing SDK source files.
  webpack(config) {
    config.resolve = config.resolve ?? {}
    config.resolve.extensionAlias = {
      ...config.resolve.extensionAlias,
      '.js': ['.ts', '.tsx', '.js'],
    }
    // Alias the SDK package name so imports from the source work in SSR too
    config.resolve.alias = {
      ...config.resolve.alias,
      '@pay/sdk': path.resolve(__dirname, '../sdk/sdk/src'),
    }
    return config
  },
}

export default nextConfig
