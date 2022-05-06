#ifndef _STATE_H
#define _STATE_H

struct State final {
  State();
  ~State();

  State& operator=(State&&);
  State(State&&);

  State(const State&) = delete;
  State& operator=(const State&) = delete;
};

#endif
