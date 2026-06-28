/** @type {import('next').NextConfig} */
const nextConfig = {
  // The browser never talks to the Rust API directly; the Next server (BFF) proxies
  // with the credential held in an httpOnly cookie. No rewrites needed.
};

export default nextConfig;
