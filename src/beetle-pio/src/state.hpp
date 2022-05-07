#ifndef _STATE_H
#define _STATE_H

#include <cstdlib>
#include <cstring>
#include <cstdint>
#include <variant>

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

struct WorkingState final {
  constexpr static const uint16_t WORKING_BUFFER_SIZE = 2048;
  explicit WorkingState(uint16_t);
  ~WorkingState();
  WorkingState(WorkingState&&);
  WorkingState& operator=(WorkingState&&);

  WorkingState(const WorkingState&) = delete;
  WorkingState& operator=(const WorkingState&) = delete;

  char * message_content;
  uint16_t message_size;

  char * id_content;
  uint16_t id_size;
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
