#include "state.hpp"

namespace states {

State::State(): active(Configuring()) {}

State::State(State&& other):
  active(std::move(other.active)) {
}

State& State::operator=(State&& other) {
  this->active = std::move(other.active);
  return *this;
}

Message::Message():
  content((char *) malloc(sizeof(char) * states::MAX_MESSAGE_SIZE)),
  size(0) {
  log_d("allocating message");
}

Message::Message(Message&& other):
  content(other.content),
  size(other.size) {
    other.size = 0;
    other.content = nullptr;
}

Message& Message::operator=(Message&& other) {
  this->content = other.content;
  this->size = other.size;

  other.content = nullptr;
  other.size = 0;

  return *this;
}

Message::~Message() {
  if (content != nullptr) {
    log_d("releasing memory content");
    free(content);
  }
}

Working::Working(uint16_t size):
  id_content((char *) malloc(sizeof(char) * states::MAX_ID_SIZE)),
  id_size(size),
  messages({}),
  _has_new(false) {
  log_d("creating working state");
  memset(id_content, '\0', states::MAX_ID_SIZE);
}

Working::Working(Working&& other): messages(std::move(other.messages)) {
  id_content = other.id_content;
  id_size = other.id_size;
  _has_new = other._has_new;
  other.id_size = 0;
  other.id_content = nullptr;
}

Working& Working::operator=(Working&& other) {
  this->id_content = other.id_content;
  this->id_size = other.id_size;
  this->_has_new = other._has_new;
  this->messages = std::move(other.messages);

  other.id_content = nullptr;
  other.id_size = 0;
  other._has_new = false;

  return *this;
}

std::array<Message, states::MESSAGE_COUNT>::const_iterator Working::end(void) const {
  return messages.cend();
}

// When requesting an iterator to our messages while in the `Working`, we will assume that subsequent
// requests are no interested in anything they have read since the last iterator was requested.
std::array<Message, states::MESSAGE_COUNT>::const_iterator Working::begin(void) const {
  return _has_new ? messages.cbegin() : messages.cend();
}

// Get a reference to the next available message.
Message& Working::next(void) {
  std::swap(messages[0], messages[states::MESSAGE_COUNT-1]);
  _has_new = true;

  for (uint8_t i = states::MESSAGE_COUNT - 1; i > 1; i--) {
    std::swap(messages[i], messages[i-1]);
  }

  messages[0].size = 0;
  memset(messages[0].content, '\0', MAX_MESSAGE_SIZE);

  return messages[0];
}

Working::~Working() {
  if (id_content != nullptr) {
    free(id_content);
  }
}

void State::freeze(void) {
  if (std::holds_alternative<Working>(this->active)) {
    Working * working_state = std::get_if<Working>(&this->active);
    working_state->_has_new = false;
  }
}

}
