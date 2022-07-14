#ifndef _STATE_H
#define _STATE_H

#include <cstdlib>
#include <cstring>
#include <cstdint>
#include <array>
#include <variant>
#include "esp32-hal-log.h"

constexpr const uint32_t MAX_MESSAGE_SIZE = 1024;

struct UnknownState final {
  UnknownState() = default;
  UnknownState(UnknownState&&) = default;
  UnknownState& operator=(UnknownState&&) = default;

  UnknownState(const UnknownState&) = delete;
  UnknownState& operator=(const UnknownState&) = delete;
};

struct ConfiguringState final {
  ConfiguringState() = default;
  ConfiguringState(ConfiguringState&&) = default;
  ConfiguringState& operator=(ConfiguringState&&) = default;

  ConfiguringState(const ConfiguringState&) = delete;
  ConfiguringState& operator=(const ConfiguringState&) = delete;
};

struct ConnectingState final {
  ConnectingState(): attempt(0) {}
  ConnectingState(uint8_t a): attempt(a) {}
  ConnectingState(ConnectingState&& other) { attempt = other.attempt; }
  ConnectingState& operator=(ConnectingState&& other) {
    this->attempt = other.attempt;
    return *this;
  }

  ConnectingState(const ConnectingState&) = delete;
  ConnectingState& operator=(const ConnectingState&) = delete;

  uint8_t attempt;
};

struct ConnectedState final {
  ConnectedState() = default;
  ~ConnectedState() = default;
  ConnectedState(ConnectedState&& other) = default;
  ConnectedState& operator=(ConnectedState&& other) = default;

  ConnectedState(const ConnectedState&) = delete;
  ConnectedState& operator=(const ConnectedState&) = delete;
};

struct Message final {
  Message();
  ~Message();

  Message(Message&& other);
  Message& operator=(Message&& other);

  Message(const Message&) = delete;
  Message& operator=(const Message&) = delete;

  char * content;
  uint32_t content_size;
};

struct WorkingState final {
  constexpr static const uint16_t WORKING_BUFFER_SIZE = 10;
  constexpr static const uint16_t MAX_ID_SIZE = 40;
  static constexpr const uint8_t MESSAGE_COUNT = 5;

  explicit WorkingState(uint16_t);
  ~WorkingState();
  WorkingState(WorkingState&&);
  WorkingState& operator=(WorkingState&&);

  WorkingState(const WorkingState&) = delete;
  WorkingState& operator=(const WorkingState&) = delete;

  char * id_content;
  uint16_t id_size;

  std::array<Message, MESSAGE_COUNT> messages;
};

using StateT = std::variant<
  UnknownState,
  ConfiguringState,
  ConnectingState,
  ConnectedState,
  WorkingState
>;

struct State final {
  State();
  ~State();

  State& operator=(State&&);
  State(State&&);

  State(const State&) = delete;
  State& operator=(const State&) = delete;

  StateT active;
};

#endif
