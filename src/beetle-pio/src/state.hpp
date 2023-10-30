#ifndef _STATE_H
#define _STATE_H 1

#include <array>
#include <memory>
#include <variant>

namespace states {
constexpr static const uint32_t BUFFER_SIZE = 1024 * 80;

struct Unknown final {};

struct Connecting final {};

struct Connected final {};

struct Configuring final {};

struct HoldingUpdate final {
  std::shared_ptr<std::array<uint8_t, BUFFER_SIZE>> buffer;
  uint32_t size;
};

typedef std::variant<Unknown, HoldingUpdate, Connected, Connecting, Configuring>
    State;
}

#endif
