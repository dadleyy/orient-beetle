#ifndef _REDIS_CONFIG_H
#define _REDIS_CONFIG_H 1

#ifdef REDIS_HOST
const char * redis_host PROGMEM = "" REDIS_HOST;
#endif

#ifdef REDIS_PORT
const unsigned int redis_port PROGMEM = REDIS_PORT;
#endif

#endif
