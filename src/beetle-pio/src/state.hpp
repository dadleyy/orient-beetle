#ifndef _STATE_H
#define _STATE_H

#include <cstdint>
#include <variant>

struct UnknownState final {
  UnknownState() {}
  UnknownState(UnknownState&&) {}
  UnknownState& operator=(UnknownState&&) { return *this; }

  UnknownState(const UnknownState&) = delete;
  UnknownState& operator=(const UnknownState&) = delete;
};

struct ConfiguringState final {
  ConfiguringState() {}
  ConfiguringState(ConfiguringState&&) {}
  ConfiguringState& operator=(ConfiguringState&&) { return *this; }

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
  ConnectedState() {}
  ConnectedState(ConnectedState&&) {}
  ConnectedState& operator=(ConnectedState&&) { return *this; }

  ConnectedState(const ConnectedState&) = delete;
  ConnectedState& operator=(const ConnectedState&) = delete;
};

using StateT = std::variant<UnknownState, ConfiguringState, ConnectingState, ConnectedState>;

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
