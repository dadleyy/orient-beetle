#pragma once

namespace redisevents {
struct RedisConfig final {
  RedisConfig(const char *host, const uint32_t port,
              std::pair<const char *, const char *> auth)
      : host(host), port(port), auth(auth) {}
  const char *host;
  const uint32_t port;
  std::pair<const char *, const char *> auth;
};
}
