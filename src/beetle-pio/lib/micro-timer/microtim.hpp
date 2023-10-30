#ifndef _MICROTIM_H
#define _MICROTIM_H

namespace microtim {

class MicroTimer final {
 public:
  explicit MicroTimer(uint16_t time)
      : _interval(time), _remaining(time), _last_time(0) {}
  ~MicroTimer() = default;

  uint8_t update(uint32_t now) {
    if (_last_time == 0 || now < _last_time) {
      _last_time = now;
      return 0;
    }

    uint32_t diff = now - _last_time;
    _last_time = now;

    if (diff >= _remaining) {
      _remaining = _interval;
      return 1;
    }

    _remaining = _remaining - diff;
    return 0;
  }

  // Disable copies.
  MicroTimer& operator=(const MicroTimer&) = delete;
  MicroTimer(const MicroTimer&) = delete;

  MicroTimer(MicroTimer&& other)
      : _interval(other._interval),
        _remaining(other._remaining),
        _last_time(other._last_time) {}

  MicroTimer& operator=(MicroTimer&& other) {
    this->_interval = other._interval;
    this->_remaining = other._remaining;
    this->_last_time = other._last_time;

    return *this;
  }

 private:
  uint32_t _interval;
  uint32_t _remaining;
  uint32_t _last_time;
};
}

#endif
