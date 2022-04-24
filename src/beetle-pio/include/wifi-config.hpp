#ifndef _WIFI_CONFIG_H
#define _WIFI_CONFIG_H 1

#ifndef WIFI_SSID
#define WIFI_SSID "orient-beetle setup"
#endif
#ifndef WIFI_PASSWORD
#define WIFI_PASSWORD "password"
#endif

const char * AP_SSID PROGMEM = WIFI_SSID;
const char * AP_PASSWORD PROGMEM = WIFI_PASSWORD;

#endif
