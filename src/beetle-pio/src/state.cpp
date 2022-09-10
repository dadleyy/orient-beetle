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

Message::Message():
  content((char *) malloc(sizeof(char) * MAX_MESSAGE_SIZE)),
  content_size(0) {
  log_d("allocating message");
}

Message::Message(Message&& other):
  content(other.content),
  content_size(other.content_size) {
    other.content_size = 0;
    other.content = nullptr;
}

Message& Message::operator=(Message&& other) {
  this->content = other.content;
  this->content_size = other.content_size;

  other.content = nullptr;
  other.content_size = 0;

  return *this;
}

Message::~Message() {
  if (content != nullptr) {
    log_d("releasing memory content");
    free(content);
  }
}

WorkingState::WorkingState(uint16_t size):
  id_content((char *) malloc(sizeof(char) * MAX_ID_SIZE)),
  id_size(size),
  messages({}) {
  log_d("creating working state");
  memset(id_content, '\0', MAX_ID_SIZE);
}

WorkingState::WorkingState(WorkingState&& other): messages(std::move(other.messages)) {
  id_content = other.id_content;
  id_size = other.id_size;
  other.id_content = nullptr;
}

std::array<Message, WorkingState::MESSAGE_COUNT>::const_iterator WorkingState::end(void) const {
  return messages.cend();
}

std::array<Message, WorkingState::MESSAGE_COUNT>::const_iterator WorkingState::begin(void) const {
  return messages.cbegin();
}

Message& WorkingState::next(void) {
  std::swap(messages[0], messages[WorkingState::MESSAGE_COUNT-1]);

  for (uint8_t i = WorkingState::MESSAGE_COUNT - 1; i > 1; i--) {
    std::swap(messages[i], messages[i-1]);
  }

  return messages[0];
}

WorkingState& WorkingState::operator=(WorkingState&& other) {
  this->id_content = other.id_content;
  this->id_size = other.id_size;

  this->messages = std::move(other.messages);

  other.id_content = nullptr;
  return *this;
}

WorkingState::~WorkingState() {
  if (id_content != nullptr) {
    free(id_content);
  }
}
