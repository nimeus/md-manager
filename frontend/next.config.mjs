/** @type {import('next').NextConfig} */
const nextConfig = {
  // The browser never talks to the Rust API directly; the Next server (BFF) proxies
  // with the credential held in an httpOnly cookie. No rewrites needed.
  //
  // `standalone` emits a self-contained server (.next/standalone/server.js) for a small
  // production Docker image — see frontend/Dockerfile.
  output: "standalone",
};

export default nextConfig;
