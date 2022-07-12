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

Message::Message() {
  log_d("allocating message");
}

WorkingState::WorkingState(uint16_t size):
  messages({}),
  message_content((char *) malloc(sizeof(char) * WORKING_BUFFER_SIZE)),
  message_size(0),
  id_content((char *) malloc(sizeof(char) * MAX_ID_SIZE)),
  id_size(size) 
{
  log_d("creating working state");
  memset(message_content, '\0', WORKING_BUFFER_SIZE);
  memset(id_content, '\0', MAX_ID_SIZE);
}

WorkingState::WorkingState(WorkingState&& other): messages(std::move(other.messages)) {
  message_content = other.message_content;
  message_size = other.message_size;

  id_content = other.id_content;
  id_size = other.id_size;

  other.message_content = nullptr;
  other.id_content = nullptr;
}

WorkingState& WorkingState::operator=(WorkingState&& other) {
  // Steal the pointers
  this->message_content = other.message_content;
  this->id_content = other.id_content;
  this->messages = std::move(other.messages);

  this->message_size = other.message_size;
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
