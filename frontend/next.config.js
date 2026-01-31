/** @type {import('next').NextConfig} */
const nextConfig = {
  async rewrites() {
    return [
      {
        source: '/v1/:path*',
        destination: 'http://localhost:8080/v1/:path*',
      },
      {
        source: '/healthz',
        destination: 'http://localhost:8080/healthz',
      },
      {
        source: '/auth/:path*',
        destination: 'http://localhost:8080/auth/:path*',
      },
    ];
  },
};

module.exports = nextConfig;
