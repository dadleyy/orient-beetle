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

WorkingState::WorkingState(uint16_t size):
  message_content((char *) malloc(sizeof(char) * WORKING_BUFFER_SIZE)),
  message_size(0),
  id_content((char *) malloc(sizeof(char) * size)),
  id_size(size) 
{
  memset(message_content, '\0', WORKING_BUFFER_SIZE);
}

WorkingState::WorkingState(WorkingState&& other) {
  message_content = other.message_content;
  message_size = other.message_size;

  id_content = other.id_content;
  id_size = other.id_size;

  other.message_content = nullptr;
  other.id_content = nullptr;
}

WorkingState& WorkingState::operator=(WorkingState&& other) {
  this->message_content = other.message_content;
  this->message_size = other.message_size;
  this->id_content = other.id_content;
  this->id_size = other.id_size;
  other.message_content = nullptr;
  other.id_content = nullptr;
  return *this;
}

WorkingState::~WorkingState() {
  if (message_content != nullptr) {
    free(message_content);
  }
  if (id_content != nullptr) {
    free(id_content);
  }
}
