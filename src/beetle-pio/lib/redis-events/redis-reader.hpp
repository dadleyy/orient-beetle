#ifndef _REDIS_READER_H
#define _REDIS_READER_H 1

namespace redisevents {

struct RedisEmpty final {};

struct RedisFailure final {};

struct RedisInt final {
  int32_t value;
};

struct RedisArray final {
  int32_t size;
};

struct RedisRead final {
  int32_t size;
};

typedef std::variant<RedisEmpty, RedisRead, RedisInt, RedisArray, RedisFailure>
    ReadEvent;

template <std::size_t T>
class RedisReader final {
 public:
  RedisReader() : collector(EmptyCollector{}) {}
  ~RedisReader() = default;

  RedisReader(RedisReader&) = delete;
  RedisReader& operator=(RedisReader&) = delete;
  RedisReader(const RedisReader&) = delete;
  RedisReader& operator=(const RedisReader&) = delete;

  ReadEvent fill(char token, std::shared_ptr<std::array<uint8_t, T>> buffer) {
    auto [next_collector, read_event] =
        std::visit(Visitor(token, buffer), collector);
    collector = next_collector;
    return read_event;
  }

 private:
  enum CollectionKind {
    String,
    Array,
    Integer,
  };

  struct EmptyCollector {};

  struct DrainCollector {
    bool terminating = false;
  };

  struct SimpleStringCollector {
    bool is_error = false;
    int32_t len = 0;
    bool terminating = false;
  };

  struct SizeCollector {
    CollectionKind kind;
    int32_t len = 0;
    bool terminating = false;
    int8_t modifier = 1;
  };

  struct StringCollector {
    StringCollector(int32_t len) : len(len) {}
    int32_t len;
    uint32_t seen = 0;
  };

  typedef std::variant<EmptyCollector, SizeCollector, StringCollector,
                       SimpleStringCollector, DrainCollector>
      Collector;

  struct Visitor {
    Visitor(char token, std::shared_ptr<std::array<uint8_t, T>> buffer)
        : token(token), buffer(buffer) {}

    std::pair<Collector, ReadEvent> operator()(
        const SimpleStringCollector e) const {
      if (token == '\r') {
        return std::make_pair(SimpleStringCollector{e.is_error, e.len, true},
                              RedisEmpty{});
      }

      if (token == '\n' && e.terminating) {
        log_i("terminated simple string message: '%s' (error? %d)",
              buffer->data(), e.is_error);
        return std::make_pair(EmptyCollector{}, RedisRead{e.len});
      }

      buffer->at(e.len) = token;
      return std::make_pair(SimpleStringCollector{e.is_error, e.len + 1, false},
                            RedisEmpty{});
    }

    std::pair<Collector, ReadEvent> operator()(const EmptyCollector) const {
      if (token == '*' || token == '$') {
        auto kind =
            token == '*' ? CollectionKind::Array : CollectionKind::String;
        log_i("has bulk array or bulk string (string? %d)", token == '$');
        return std::make_pair(SizeCollector{kind, 0, false}, RedisEmpty{});
      }

      if (token == ':') {
        return std::make_pair(SizeCollector{CollectionKind::Integer, 0, false},
                              RedisEmpty{});
      }

      if (token == '-' || token == '+') {
        log_i("has simple string");
        return std::make_pair(SimpleStringCollector{token == '-', 0, false},
                              RedisEmpty{});
      }

      log_i("unrecognized token: '%c'", token);

      return std::make_pair(EmptyCollector{}, RedisEmpty{});
    }

    std::pair<Collector, ReadEvent> operator()(
        const SizeCollector collector) const {
      if (token == '\r') {
        return std::make_pair(
            SizeCollector{collector.kind, collector.len, true}, RedisEmpty{});
      }

      if (token == '\n' && collector.terminating) {
        if (collector.kind == CollectionKind::Array) {
          return std::make_pair(EmptyCollector{}, RedisArray{collector.len});
        }

        if (collector.kind == CollectionKind::Integer) {
          return std::make_pair(EmptyCollector{}, RedisInt{collector.len});
        }

        buffer->fill('\0');

        if (collector.len < 0) {
          log_e("received a negative string size '%d', ignoring",
                collector.len);
          return std::make_pair(EmptyCollector{}, RedisEmpty{});
        }

        log_i("finished bulk string size collection: %d", collector.len);
        return std::make_pair(StringCollector(collector.len), RedisEmpty{});
      }

      // If the first token is `-`, flip our modifier, retaining the len 0
      if (token == '-') {
        return std::make_pair(SizeCollector{collector.kind, 0, false, -1},
                              RedisEmpty{});
      }

      log_d("adding '%c' to size collector len", token);
      auto amount = (token - '0');
      return std::make_pair(
          SizeCollector{collector.kind,
                        (collector.len * 10) + (amount * collector.modifier),
                        false},
          RedisEmpty{});
    }

    std::pair<Collector, ReadEvent> operator()(
        const DrainCollector collector) const {
      if (token == '\r' && !collector.terminating) {
        return std::make_pair(DrainCollector{true}, RedisEmpty{});
      }
      if (token == '\n' && collector.terminating) {
        return std::make_pair(EmptyCollector{}, RedisEmpty{});
      }

      log_e("expected a drain, but received '%c'", token);
      return std::make_pair(EmptyCollector{}, RedisEmpty{});
    }

    std::pair<Collector, ReadEvent> operator()(
        const StringCollector collector) const {
      if (T <= collector.seen) {
        log_e("not enough space for message!");
        return std::make_pair(EmptyCollector{}, RedisFailure{});
      }

      buffer->at(collector.seen) = token;

      if (collector.seen + 1 == collector.len) {
        log_i("completely parsed %d of %d bulk string", collector.seen,
              collector.len);
        return std::make_pair(DrainCollector{false}, RedisRead{collector.len});
      }

      auto next = StringCollector(collector.len);
      next.seen = collector.seen + 1;
      return std::make_pair(next, RedisEmpty{});
    }

    char token;
    std::shared_ptr<std::array<uint8_t, T>> buffer;
  };

  Collector collector;
};
}  // namespace redisevents

#endif
