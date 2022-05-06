#include "state.hpp"

State::State(): active(ConfiguringState()) {
}

State::~State() {
}

State::State(State&& other): active(std::move(other.active)) {
}

State& State::operator=(State&& other) {
  this->active = std::move(other.active);
  return *this;
}
