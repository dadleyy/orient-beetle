#pragma once

#include <variant>

namespace redisevents {

struct PayloadReceived final {
  uint32_t size;
};

struct IdentificationReceived final {};

struct Authorized final {};

struct FailedConnection final {};

typedef std::variant<PayloadReceived, Authorized, FailedConnection,
                     IdentificationReceived>
    RedisEvent;
}
