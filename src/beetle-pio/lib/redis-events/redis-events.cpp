#include "redis-events.hpp"

namespace redisevents {

  Events::Events(std::tuple<const char *, const uint32_t, std::pair<const char *, const char *>> config):
    _redis_host(std::get<0>(config)),
    _redis_port(std::get<1>(config)),
    _redis_auth(std::get<2>(config)),
    _paused(false),
    _state(Disconnected {})
  {
  }

  void Events::begin(void) {
    _preferences.begin("beetle-redis", false);
  }

  // Given a destination, fill it with the contents of our connected state if available.
  uint16_t Events::copy(char * destination, uint16_t max) {
    if (_state.index() != 1) {
      return 0;
    }

    Connected * connected_state = std::get_if<1>(& _state);
    return connected_state->copy(destination, max);
  }

  // The main "tick" function of our redis manager.
  std::optional<Events::EMessage> Events::update(
    std::optional<wifievents::Events::EMessage> &message,
    uint32_t current_time
  ) {
    switch (_state.index()) {
      // When disconnected - check to see if the wifi manager has connected.
      case 0: {
        Disconnected * disconnected_state = std::get_if<0>(&_state);

        if (disconnected_state->update(message) || _paused) {
          log_i("attempting to move from disconnect to connected");

          _paused = false;
          _state.emplace<Connected>(&_preferences);
        }
        break;
      }

      // If we're connected, continue this frame by attempting an update from the connected state.
      // We'll also be handling wifi disconnects in this state.
      case 1: {
        Connected * connected_state = std::get_if<1>(&_state);

        // If we've been completely disconnected, restart.
        if (message == wifievents::Events::EMessage::Disconnected) {
          _state.emplace<Disconnected>();
          _paused = false;
          return Events::EMessage::ConnectionLost;
        }

        // If we're paused and we've got anything but a resume, do nothing.
        if (_paused && message != wifievents::Events::EMessage::ConnectionResumed) {
          return std::nullopt;
        }

        // If we've received an interruption, pause immediately.
        if (message == wifievents::Events::EMessage::ConnectionInterruption) {
          _paused = true;
          log_i("wifi connection was interrupted, pausing all requests");

          return std::nullopt;
        }

        // If we're paused and are about to resume, reset us to disconnected. The next frame will
        // bounce us back into connected.
        if (_paused && message == wifievents::Events::EMessage::ConnectionResumed) {
          _state.emplace<Disconnected>();
          return Events::EMessage::ConnectionLost;
        }

        // At this point, the message received from the wifi manager is not something we're really
        // concerned with, so apply our update.
        return connected_state->update(_redis_host, _redis_auth, _redis_port, current_time);
      }
    }

    return std::nullopt;
  }

  // Connected state copy - consume the framebuffer into our destination.
  uint16_t Events::Connected::copy(char * destination, uint16_t size) {
    if (_cursor == 0 || _authorization_stage != EAuthorizationStage::FullyAuthorized) {
      log_e("attempted to copy an empty message");
      return 0;
    }

    uint16_t amount = _cursor < size ? _cursor : size;

    log_i("copying (size %d) message", amount);

    memcpy(destination, _framebuffer, amount);
    _cursor = 0;
    memset(_framebuffer, '\0', FRAMEBUFFER_SIZE);
    return amount;
  }

  // The main update method here is responsible for potentially receiving and writing
  // data over tcp to our redis server.
  std::optional<Events::EMessage> Events::Connected::update(
    const char * _redis_host,
    const std::pair<const char *, const char *> & _redis_auth,
    uint32_t _redis_port,
    uint32_t current_time
  ) {
    _cursor = 0;

    if (_strange_thing_count > 10) {
      log_e("too many strange things have happened. resetting the tcp connection");
      reset();
      return std::nullopt;
    }

    if (_timer.update(current_time) != 1) {
      return std::nullopt;
    }

    switch (_authorization_stage) {
      // If we have not attempted to connect + certify, we need to apply our root certificates
      // and open the tcp connection using our `client.`
      case EAuthorizationStage::NotRequested: {
        return connect(_redis_host, _redis_auth, _redis_port);
      }

      // If we have received an `+OK` from our burn-in credential authorization attempt, time to
      // request an id by writing our pop.
      case EAuthorizationStage::AuthorizationReceived: { 
        log_i("[id request] requesting new id from registrar using burn-in credentials");
        _authorization_stage = EAuthorizationStage::IdentificationRequested;
        _client.print(REDIS_REGISTRATION_POP);
        return std::nullopt;
      }

      // States that are expecting to read _something_ off the tcp connection.
      case IdentificationRequested:
      case AuthorizationAttempted:
      case AuthorizationRequested:
      case FullyAuthorized:
        break;
    }

    // Clear out the framebuffer.
    memset(_framebuffer, '\0', FRAMEBUFFER_SIZE);

    // Read everything we can off our client.
    while (_client.available() && _cursor < FRAMEBUFFER_SIZE - 1) {
      // TODO: `_client.read((uint8_t *) _framebuffer, FRAMEBUFFER)` might be a better choice here
      _framebuffer[_cursor] = (char) _client.read();
      _cursor += 1;
    }

    // Explicitly handle `wrongpass`.
    if (strcmp(_framebuffer, WRONG_PASS_ERR) == 0) {
      log_e("wrongpass received, resetting client");
      reset();

      return std::nullopt;
    }

    // Explicitly handle bad permission errors.
    if (strcmp(_framebuffer, NO_PERM_ERR) == 0) {
      log_e("permissions lost, resetting client");
      reset();

      return std::nullopt;
    }

    bool is_empty = 
      strcmp(_framebuffer, EMPTY_STRING_RESPONSE) == 0 || strcmp(_framebuffer, EMPTY_ARRAY_RESPONSE) == 0;
    bool is_ok = strcmp(_framebuffer, OK) == 0;

    switch (_authorization_stage) {
      // Neither of these states should be dealing with parsing incoming messages from
      // our tcp stream.
      case EAuthorizationStage::NotRequested:
        return std::nullopt;

      case EAuthorizationStage::AuthorizationReceived:
        return std::nullopt;

      case EAuthorizationStage::AuthorizationAttempted: {
        // The only thing we're expecting here is an `+OK`. Anyhing else means something strange
        // has happened.
        if (is_ok) {
          log_i("received 'OK' during certification stage '%d'", _authorization_stage);
          _authorization_stage = EAuthorizationStage::FullyAuthorized;
          _strange_thing_count = 0;
          return std::nullopt;
        }

        if (_cursor > 0) {
          log_e("received strange response after attempting device-specific acl: '%s'", _framebuffer);
          _strange_thing_count = _strange_thing_count + 1;
        }

        return std::nullopt;
      } 

      case EAuthorizationStage::AuthorizationRequested: {
        // The only thing we're expecting here is an `+OK`. Anyhing else means something strange
        // has happened.
        if (is_ok) {
          log_i("successfully authorized connection to redis, will attempt to pull id on next update");
          _authorization_stage = EAuthorizationStage::AuthorizationReceived;
          _strange_thing_count = 0;
          return Events::EMessage::EstablishedConnection;
        }

        if (_cursor > 0) {
          log_e("received strange response after attempting burn-in acl: '%s'", _framebuffer);
          _strange_thing_count = _strange_thing_count + 1;
        }

        return std::nullopt;
      }
      
      case EAuthorizationStage::FullyAuthorized: {
        // If nothing was pulled this frame, we either want to increment our count of
        // "hey I was expecting a message", or we want to write a new message.
        if (_cursor == 0) {
          if (_pending_response) {
            _empty_identified_reads += 1;
          } else {
            write_message(current_time);
          }
          return std::nullopt;
        }

        // Short circuit any actual parsing if we have a well known response.
        auto parse_result = is_empty ? EParseResult::ParsedOk : parse_framebuffer();

        switch (parse_result) {
          case EParseResult::ParsedNothing:
            _strange_thing_count = 0;
            return std::nullopt;
          case EParseResult::ParsedOk:
            _empty_identified_reads = 0;
            _strange_thing_count = 0;
            _pending_response = false;
            log_i("received some form of acknowlegement, writing next message");
            write_message(current_time);
            return std::nullopt;
          case EParseResult::ParsedMessage:
            log_i("message parsed successfully");
            _strange_thing_count = 0;
            if (strcmp(_framebuffer, "__reset__") == 0) {
              _cached_reset_count = MAX_RESETS_RECREDENTIALIZE + 1;
              reset();
              return std::nullopt;
            }
            _empty_identified_reads = 0;
            _pending_response = false;
            write_message(current_time);
            return EMessage::ReceivedMessage;
          default:
            // Reset our cursor; we don't want it looking like we have a message.
            _cursor = 0;
            _pending_response = false;
            log_e("strange parse result while authorized - '%s'", _framebuffer);
            _strange_thing_count = _strange_thing_count + 1;
            return std::nullopt;
        }
      }

      case EAuthorizationStage::IdentificationRequested: {
        if (is_empty) {
          log_e("empty response from identification request, is registrar running?");
          _strange_thing_count = _strange_thing_count + 1;
          return std::nullopt;
        }

        auto parse_result = _cursor > 0 ? parse_framebuffer() : EParseResult::ParsedNothing;

        switch (parse_result) {
          case EParseResult::ParsedNothing: {
            return std::nullopt;
          }

          // If we have a message while waiting for an id, we're going to assume that the framebuffer
          // is now ready with our contents.
          case EParseResult::ParsedMessage: {
            log_i("[id received] assuming '%s' is our identity", _framebuffer);
            _device_id_len = _cursor;
            memcpy(_device_id, _framebuffer, _device_id_len);
            size_t stored_id_len = _preferences->putString("device-id", _device_id);
            log_d("stored device id (%d bytes)", stored_id_len);
            // We now have our individual identity.
            _authorization_stage = EAuthorizationStage::FullyAuthorized;
            log_i("writing auth command with new id %s (%d bytes)", _device_id, _device_id_len);
            // Now that we have a valid device id, authorize as that.
            memset(_outbound_buffer, '\0', OUTBOUND_BUFFER_SIZE);
            sprintf(
              _outbound_buffer,
              "*3\r\n$4\r\nAUTH\r\n$%d\r\n%s\r\n$%d\r\n%s\r\n",
              _device_id_len,
              _device_id,
              _device_id_len,
              _device_id
            );
            log_d("---auth\n%s\n", _outbound_buffer);
            _client.print(_outbound_buffer);
            memset(_outbound_buffer, '\0', OUTBOUND_BUFFER_SIZE);
            _authorization_stage = EAuthorizationStage::AuthorizationAttempted;
            return Events::EMessage::IdentificationReceived;
          }
          default: {
            _strange_thing_count = _strange_thing_count + 1;
            log_e("failed to receive anything while waiting for an id (framebuffer %s)", _framebuffer);
            return std::nullopt;
          }
        }
      }
    }

    return std::nullopt;
  }

  // The primary redis response parsing function. Called whenever our current framebuffer is full.
  inline Events::EParseResult Events::Connected::parse_framebuffer(void) {
    memset(_parse_buffer, '\0', PARSED_MESSAGE_SIZE);

    char * current_token = _framebuffer;
    uint32_t current_index = 0, last_string_start = 0, tokens_read = 0, last_string_end = 0;
    bool capturing = false, done = false;

    while (*current_token != '\0' && !done) {
      auto tok = *current_token;
      current_token++;
      auto transition = _parser.consume(tok);
      tokens_read += 1;

      switch (transition) {
        case Events::EResponseParserTransition::Noop: {
          if (capturing) {
            _parse_buffer[current_index] = tok;
            current_index ++;
          }
          continue;
        }

        case Events::EResponseParserTransition::Failure: {
          memset(_framebuffer, '\0', FRAMEBUFFER_SIZE);
          log_e("aborting all parsing, failure received");
          return Events::EParseResult::ParsedFailure;
        }

        // If we can immediately exit parsing because we've received an empty array or bulk string,
        // we can continue sending our next request.
        case Events::EResponseParserTransition::Done: {
          log_e("strange framebuffer parsing termination");
          memset(_framebuffer, '\0', FRAMEBUFFER_SIZE);
          return Events::EParseResult::ParsedOk;
        }

        case Events::EResponseParserTransition::HasArray: {
          capturing = false;
          log_i("starting array capture");
          continue;
        }

        // At the start of every bulk string, reset our parsed buffer and flag the start
        // of this capture.
        case Events::EResponseParserTransition::StartString: {
          log_d("starting string capture");
          memset(_parse_buffer, '\0', PARSED_MESSAGE_SIZE);
          capturing = true;
          current_index = 0;
          last_string_start = tokens_read;
          continue;
        }

        // When we've reached the end of our string, we're going to fully swap our parsed
        // message contents into the framebuffer.
        case Events::EResponseParserTransition::EndString: {
          // TODO: the way the state transitions work currently we are sending a single '\r'
          // character into the `_parse_buffer`.
          auto terminal_index = current_index - 1;
          last_string_end = last_string_start + terminal_index;

          assert(terminal_index <= FRAMEBUFFER_SIZE);

          // Note: This assignment is used later when copying messages out of the connected state.
          // That is a pretty gnarly way to go about doing things;
          _cursor = terminal_index;

          log_i(
            "finishing string capture - '%s' (located @ %d -> %d)",
            _parse_buffer,
            last_string_start,
            last_string_end
          );
          capturing = false;
          continue;
        }
      }
    }

    // There is definitely a better way to do this...
    if (last_string_end > 0) {
      memset(_parse_buffer, '\0', PARSED_MESSAGE_SIZE);
      memcpy(_parse_buffer, _framebuffer + last_string_start, _cursor);
      log_i("final message parsed - '%s' (%d chars)", _parse_buffer, _cursor);
      memset(_framebuffer, '\0', FRAMEBUFFER_SIZE);
      memcpy(_framebuffer, _parse_buffer, _cursor);

      return EParseResult::ParsedMessage;
    }

    return EParseResult::ParsedNothing;
  }

  Events::EResponseParserTransition Events::ResponseParser::consume(char token) {
    auto [state, transition] = std::visit(Events::ParserVisitor(token), std::move(_state));
    _state = std::move(state);
    return transition;
  }

  Events::Connected::Connected(Preferences* _preferences):
    _authorization_stage(EAuthorizationStage::NotRequested),
    _cursor(0),
    _parser(ResponseParser()),
    _framebuffer((char*) malloc(sizeof(char) * FRAMEBUFFER_SIZE)),
    _outbound_buffer((char*) malloc(sizeof(char) * OUTBOUND_BUFFER_SIZE)),
    _parse_buffer((char*) malloc(sizeof(char) * PARSED_MESSAGE_SIZE)),
    _device_id_len(0),
    _empty_identified_reads(0),
    _cached_reset_count(0),
    _preferences(_preferences) {
      _device_id = (char*) malloc(sizeof(char) * MAX_ID_SIZE);
      memset(_framebuffer, '\0', FRAMEBUFFER_SIZE);
      memset(_device_id, '\0', MAX_ID_SIZE + 1);
  }

  // Called when we're in a `NotRequested` state, this method is responsible for opening our
  // tcp connection with the correct certificate and issuing our authorization message.
  std::optional<Events::EMessage> Events::Connected::connect(
    const char * _redis_host,
    const std::pair<const char *, const char *> & _redis_auth,
    uint32_t _redis_port
  ) {
    log_i("attempting to certify redis connection");

    // TODO: figure out how to maybe read this elsewhere to make this library more portable?
    extern const uint8_t redis_root_ca[] asm("_binary_embeds_redis_host_root_ca_pem_start");

    _client.setCACert((char *) redis_root_ca);
    log_i("attempting to establish connection with redis %s:%d", _redis_host, _redis_port);

    int result = _client.connect(_redis_host, _redis_port);

    if (result != 1) {
      log_e("unable to establish connection - %d", result);
      return Events::EMessage::FailedConnection;
    }

    log_i("redis connection established successfully");

    size_t stored_id_len = _preferences->isKey("device-id")
      ?  _preferences->getString("device-id", _device_id, MAX_ID_SIZE)
      : 0;

    // If we have a stored device id in our preferences/persistent memory, attempt to authorize
    // with it and move into the authorization requested stage.
    if (stored_id_len > 0) {
      _connected_with_cached_id = true;
      _device_id_len = stored_id_len - 1;

      log_i("has stored device id '%s' (%d bytes), trying it out.", _device_id, _device_id_len);

      // Now that we have a valid device id, authorize as that.
      memset(_outbound_buffer, '\0', OUTBOUND_BUFFER_SIZE);
      sprintf(
        _outbound_buffer,
        "*3\r\n$4\r\nAUTH\r\n$%d\r\n%s\r\n$%d\r\n%s\r\n",
        _device_id_len,
        _device_id,
        _device_id_len,
        _device_id
      );
      log_d("---auth\n%s\n", _outbound_buffer);
      _client.print(_outbound_buffer);
      memset(_outbound_buffer, '\0', OUTBOUND_BUFFER_SIZE);

      // Here we are skipping right over `AuthorizationInitiated`; we had a device id in our
      // cache and attempted to use it.
      _authorization_stage = EAuthorizationStage::AuthorizationAttempted;

      return Events::EMessage::IdentificationReceived;
    }

    // So we've connected without stored credentials. What we'll do now is attempt to authenticate
    // with the burned-in ACL information which should only have the ability to pop an id.
    _authorization_stage = EAuthorizationStage::AuthorizationRequested;

    // At this point we have a valid tcp connection using tls and can start writing our redis
    // commands. Start by sending the `AUTH` command using the device id consumer ACL that should
    // be safe for global use.
    memset(_outbound_buffer, '\0', OUTBOUND_BUFFER_SIZE);
    const char * redis_username = std::get<0>(_redis_auth);
    const char * redis_password = std::get<1>(_redis_auth);
    auto name_len = strlen(redis_username);
    auto pass_len = strlen(redis_password);

    sprintf(
      _outbound_buffer,
      "*3\r\n$4\r\nAUTH\r\n$%d\r\n%s\r\n$%d\r\n%s\r\n",
      name_len,
      redis_username,
      pass_len,
      redis_password
    );
    log_d("---auth request:\n%s\n---done", _outbound_buffer);
    // TODO: `_client.write(...)` might be a better choice here
    size_t written = _client.print(_outbound_buffer);
    log_i("requested authenticated session w/ burn-in creds (%d bytes)", written);

    return std::nullopt;
  }

  void Events::Connected::reset(void) {
    _strange_thing_count = 0;
    _empty_identified_reads = 0;
    _cached_reset_count += 1;
    _pending_response = false;

    _authorization_stage = EAuthorizationStage::NotRequested;
    _client.stop();

    if (_cached_reset_count > MAX_RESETS_RECREDENTIALIZE) {
      _cached_reset_count = 0;
      log_e("client resets without successfull message exceeded max; removing device id");
      _preferences->remove("device-id");
    }

    _cursor = 0;
    memset(_framebuffer, '\0', FRAMEBUFFER_SIZE);
  }

  Events::Connected::~Connected() {
    log_i("cleaning up redis client connection");

    free(_framebuffer);
    free(_parse_buffer);
    free(_device_id);
    free(_outbound_buffer);

    _client.stop();
  }

  inline uint16_t Events::Connected::write_message(uint32_t current_time) {
    if (_write_timer.update(current_time) != 1) {
      return 0;
    }

    _pending_response = true;
    memset(_outbound_buffer, '\0', OUTBOUND_BUFFER_SIZE);
    if (_last_written_pop == false) {
      _last_written_pop = true;
      uint8_t keysize = strlen(_device_id) + 3;
      sprintf(_outbound_buffer, "*3\r\n$5\r\nBLPOP\r\n$%d\r\nob:%s\r\n$1\r\n5\r\n", keysize, _device_id);
      log_i("writing message queue pop");
    } else {
      log_i("writing diagnostic push");
      _last_written_pop = false;
      sprintf(_outbound_buffer, "*3\r\n$5\r\nRPUSH\r\n$4\r\nob:i\r\n$%d\r\n%s\r\n", strlen(_device_id), _device_id);
    }

    auto bytes_sent = _client.print(_outbound_buffer);
    memset(_outbound_buffer, '\0', OUTBOUND_BUFFER_SIZE);
    return bytes_sent;
  }

  bool Events::Disconnected::update(std::optional<wifievents::Events::EMessage> &message) {
    return message == wifievents::Events::EMessage::Connected;
  }

  uint8_t Events::id_size(void) {
    Connected * c = std::get_if<Connected>(&_state);
    if (!c) {
      return 0;
    }
    return c->_device_id_len;
  }

  uint8_t Events::copy_id(char * dest, uint8_t max) {
    Connected * c = std::get_if<Connected>(&_state);
    if (!c) {
      return 0;
    }
    uint8_t amount = c->_device_id_len < max ? c->_device_id_len : max;
    memcpy(dest, c->_device_id, amount);
    return amount;
  }

  std::tuple<Events::ParserStates, Events::EResponseParserTransition>
  Events::ParserVisitor::operator()(const Events::InitialParser&& initial) const {
    if (initial._kind == 0) {
      switch (_token) {
        case '*':
          initial._kind = 1;
          return std::make_tuple(std::move(initial), Events::EResponseParserTransition::Noop);
        case '$':
          initial._kind = 2;
          return std::make_tuple(std::move(initial), Events::EResponseParserTransition::Noop);
        case ':':
          initial._kind = 3;
          return std::make_tuple(std::move(initial), Events::EResponseParserTransition::Noop);
        default:
          return std::make_tuple(InitialParser(), Events::EResponseParserTransition::Failure);
      }
    }

    if (initial._kind == 1 && initial._total == 0 && _token == '-') {
      log_i("parsed an empty array message");
      return std::make_tuple(InitialParser(), Events::EResponseParserTransition::Done);
    }

    // If we receive newline, we should check to make sure the last character we've received was
    // a carriage return; by checking our '_delim'.
    if (_token == '\n') {
      // If we are _not_ expecting a newline, this was a poorly formatted request. Bail out.
      if (initial._delim != 1) {
        return std::make_tuple(InitialParser(), Events::EResponseParserTransition::Failure);
      }

      switch (initial._kind) {
        case 1:
          log_i("finished parsing array message length chunk (found %d)", initial._total);
          return std::make_tuple(InitialParser(), Events::EResponseParserTransition::HasArray);
        case 2:
          log_d("finished parsing bulk string message length chunk (found %d)", initial._total);
          return std::make_tuple(
            BulkStringParser(initial._total),
            Events::EResponseParserTransition::StartString
          );
        case 3:
          log_d("finished parsing int response %d", initial._total);
          return std::make_tuple(InitialParser(), Events::EResponseParserTransition::Done);
        default:
          log_e("invalid initial size chunk parsing (found %d)", initial._total);
          return std::make_tuple(InitialParser(), Events::EResponseParserTransition::Failure);
      }
    }

    // If we receive a return, mark ready to receive newline.
    if (_token == '\r') {
      initial._delim = 1;
      return std::make_tuple(std::move(initial), Events::EResponseParserTransition::Noop);
    }

    // Any other character, make sure we're not looking for a newline, and add the character to
    // our current length.
    initial._delim = 0;
    initial._total = (initial._total * 10) + (_token - '0');
    return std::make_tuple(std::move(initial), Events::EResponseParserTransition::Noop);
  }

  // When consuming tokens while parsing a bulk string, all we need to do is check for return
  // and newline characters, while making sure that we are under the expected length.
  std::tuple<Events::ParserStates, Events::EResponseParserTransition>
  Events::ParserVisitor::operator()(const Events::BulkStringParser&& initial) const {
    // If we have a newline and were previously terminating, we should be done. Move back into
    // an initial parser state to perpare for any new length bits.
    if (_token == '\n' && initial._terminating) {
      log_d("bulk string read complete");
      return std::make_tuple(InitialParser(), Events::EResponseParserTransition::EndString);
    }

    // If we have a return, we should expect to be terminating soon.
    if (_token == '\r') {
      initial._terminating = true;
      return std::make_tuple(std::move(initial), Events::EResponseParserTransition::Noop);
    }

    // Only increment our count
    auto seen = initial._seen + 1;

    if (seen > initial._size) {
      log_e("bulk string parser saw more bytes than expected");
      return std::make_tuple(std::move(initial), Events::EResponseParserTransition::Failure);
    }

    initial._seen = seen;
    return std::make_tuple(std::move(initial), Events::EResponseParserTransition::Noop);
  }
}
