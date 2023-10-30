#pragma once

#include <variant>

namespace redisevents {

struct PayloadReceived {
  uint32_t size;
};

struct IdentificationReceived {};

struct Authorized {};

struct FailedConnection {};

typedef std::variant<PayloadReceived, Authorized, FailedConnection,
                     IdentificationReceived>
    RedisEvent;
}
