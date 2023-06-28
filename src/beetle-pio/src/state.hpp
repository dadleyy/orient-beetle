#ifndef _STATE_H
#define _STATE_H 1

#include <cstdlib>
#include <cstring>
#include <cstdint>
#include <array>
#include <variant>
#include "esp32-hal-log.h"

namespace states {

// The size of our allocated memory per message.
constexpr const uint32_t MAX_MESSAGE_SIZE = 1024 * 20;

// The size of our allocated memory for the device id.
constexpr const uint16_t MAX_ID_SIZE = 40;

// The amount of messages to maintain at any given point of time.
constexpr const uint8_t MESSAGE_COUNT = 1;

// Forward declarations used for `friend` related things.
struct State;
struct Unknown;
struct Configuring;
struct Connecting;
struct Connected;
struct Working;

using StateT = std::variant<Unknown, Configuring, Connecting, Connected, Working>;

// Reserve a state to deal with displaying to the user some fatal looking screen.
struct Unknown final {
  Unknown() = default;
  Unknown(Unknown&&) = default;
  Unknown& operator=(Unknown&&) = default;

  Unknown(const Unknown&) = delete;
  Unknown& operator=(const Unknown&) = delete;
};

// This state represents where we are while waiting for the user to set of the wifi
// credentials through the "capture portal".
struct Configuring final {
  Configuring() = default;
  ~Configuring() = default;
  Configuring(Configuring&&) = default;
  Configuring& operator=(Configuring&&) = default;

  Configuring(const Configuring&) = delete;
  Configuring& operator=(const Configuring&) = delete;
};

struct Connecting final {
  Connecting(): attempt(0) {}
  explicit Connecting(uint8_t a): attempt(a) {};

  ~Connecting() = default;

  Connecting(Connecting&& other): attempt(other.attempt) { other.attempt = 0; }
  Connecting& operator=(Connecting&& other) {
    this->attempt = other.attempt;
    other.attempt = 0;
    return *this;
  }

  Connecting(const Connecting&) = delete;
  Connecting& operator=(const Connecting&) = delete;

  uint8_t attempt;
};

// A brief state - represents waiting for redis after connecting to the internet.
struct Connected final {
  Connected() = default;
  ~Connected() = default;
  Connected(Connected&& other) = default;
  Connected& operator=(Connected&& other) = default;

  Connected(const Connected&) = delete;
  Connected& operator=(const Connected&) = delete;
};

struct Message final {
  Message();
  ~Message();

  Message(Message&& other);
  Message& operator=(Message&& other);

  Message(const Message&) = delete;
  Message& operator=(const Message&) = delete;

  char * content;
  uint32_t size;
};

struct Working final {
  public:
    explicit Working(uint16_t);
    ~Working();
    Working(Working&&);
    Working& operator=(Working&&);

    Working(const Working&) = delete;
    Working& operator=(const Working&) = delete;

    std::array<Message, states::MESSAGE_COUNT>::const_iterator begin(void) const;
    std::array<Message, states::MESSAGE_COUNT>::const_iterator end(void) const;
    Message& next(void);

    char * id_content;
    uint16_t id_size;

  private:
    std::array<Message, states::MESSAGE_COUNT> messages;
    mutable bool _has_new;

    friend State;
};


struct State final {
  State();
  ~State() = default;

  State& operator=(State&&);
  State(State&&);

  State(const State&) = delete;
  State& operator=(const State&) = delete;

  void freeze(void);

  StateT active;
};

State clear_render(State &&old_state);

}

#endif
