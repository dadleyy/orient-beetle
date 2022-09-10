#include <Arduino.h>

#include "Adafruit_VCNL4010.h"
#include "TFT_eSPI.h"
#include "lvgl.h"
#include "font/lv_font.h"
#include "jelle.h"
#include "icon_font.h"

// Internal libraries
#include "microtim.hpp"
#include "wifi-manager.hpp"
#include "redis-manager.hpp"

// Configuration files
#include "board_layout.hpp"
#include "wifi_config.hpp"
#include "redis_config.hpp"

#include "engine.hpp"
#include "state.hpp"
// #include "view.hpp"

extern const char * ap_ssid;
extern const char * ap_password;

extern const char * redis_host;
extern const uint32_t redis_port;
extern const char * redis_auth_username;
extern const char * redis_auth_password;

LV_FONT_DECLARE(jelle);
// LV_FONT_DECLARE(icon_font);

// TODO: explore constructing the wifi + redis managers here. Dealing with the copy
// and/or movement semantics of their constructors is out of scope for now.
Engine eng(
  std::make_pair(ap_ssid, ap_password),
  std::make_tuple(redis_host, redis_port, std::make_pair(redis_auth_username, redis_auth_password))
);

State state;

// Rendering constructs:
TFT_eSPI tft(TFT_WIDTH, TFT_HEIGHT);
static lv_disp_drv_t disp_drv;
static lv_disp_draw_buf_t draw_buf;
static lv_color_t buf[TFT_WIDTH * 10];

static lv_style_t screen_style;
lv_obj_t* screen;

lv_obj_t * status_column;
static lv_style_t label_style;
lv_obj_t* status_label;
static lv_style_t icon_label_style;
lv_obj_t* status_icon_label;

lv_obj_t * message_row;

constexpr const uint8_t label_count = 4;
lv_obj_t* message_labels[label_count];

Adafruit_VCNL4010 vcnl;

#ifndef RELEASE
microtim::MicroTimer _debug_timer(5000);
#endif

uint32_t last_frame = 0;
bool failed = false;
bool prox_ready = false;

void view_debug(const char * view_log) {
  log_d("lvgl: %s", view_log);
}

void display_flush(lv_disp_drv_t *disp, const lv_area_t *area, lv_color_t *color_p) {
  uint32_t w = ( area->x2 - area->x1 + 1 );
  uint32_t h = ( area->y2 - area->y1 + 1 );

  tft.startWrite();
  tft.setAddrWindow(area->x1, area->y1, w, h);
  tft.pushPixelsDMA((uint16_t *) &color_p->full, w * h);
  tft.endWrite();

  lv_disp_flush_ready(disp);
}

void setup(void) {
#ifndef RELEASE
  Serial.begin(115200);
#endif

  pinMode(LED_BUILTIN, OUTPUT);

  // Turn off the display while booting.
  pinMode(LCD_PIN_NUM_BCKL, OUTPUT);
  digitalWrite(LCD_PIN_NUM_BCKL, LOW);

  unsigned char i = 0;

  while (i < 12) {
    digitalWrite(LED_BUILTIN, i % 2 == 0 ? HIGH : LOW);
    delay(500);
    i += 1;
  }

  tft.begin();
  tft.setRotation(3);
  // tft.invertDisplay(true);

  if (tft.initDMA() != 1) {
    failed = true;
    log_e("unable to initialize tft screen direct memory access");
    return;
  }

  log_i("tft screen ready, initializing lvgl");

  //
  // lvgl initialization.
  //

  lv_init();
  lv_log_register_print_cb(view_debug);
  lv_disp_draw_buf_init(&draw_buf, buf, NULL, TFT_WIDTH * 10);

  lv_disp_drv_init(&disp_drv);
  disp_drv.hor_res = TFT_WIDTH;
  disp_drv.ver_res = TFT_HEIGHT;
  disp_drv.draw_buf = &draw_buf;
  disp_drv.flush_cb = display_flush;
  lv_disp_drv_register(&disp_drv);

  //
  // Style allocation
  // Create our screen style, attach it to the screen.
  lv_style_init(&screen_style);
  lv_style_set_bg_color(&screen_style, lv_color_make(0x00, 0x00, 0x00));
  lv_style_set_border_width(&screen_style, 0);
  lv_style_set_outline_width(&screen_style, 0);

  // Create our text styles (one for icons, one for text);
  lv_style_init(&icon_label_style);
  lv_style_set_text_color(&icon_label_style, lv_color_make(0xfe, 0xfe, 0xfe));
  lv_style_set_text_font(&icon_label_style, &icon_font);

  lv_style_init(&label_style);
  lv_style_set_text_color(&label_style, lv_color_make(0xfe, 0xfe, 0xfe));
  lv_style_set_text_font(&label_style, &jelle);

  //
  // Object allocation
  screen = lv_obj_create(NULL);
  lv_obj_add_style(screen, &screen_style, 0);
  lv_obj_set_scrollbar_mode(screen, LV_SCROLLBAR_MODE_OFF);

  // Message section allocations.
  message_row = lv_obj_create(screen);
  lv_obj_set_size(message_row, TFT_WIDTH, TFT_HEIGHT - 30);
  lv_obj_set_scrollbar_mode(message_row, LV_SCROLLBAR_MODE_OFF);
  lv_obj_add_style(message_row, &screen_style, 0);
  lv_obj_align(message_row, LV_ALIGN_TOP_MID, 0, 5);
  lv_obj_set_flex_flow(message_row, LV_FLEX_FLOW_COLUMN);

  for (uint8_t i = 0; i < label_count; i++) {
    message_labels[i]= lv_label_create(message_row);
    lv_obj_align(message_labels[i], LV_ALIGN_TOP_LEFT, 0, 0);
    lv_obj_add_style(message_labels[i], &label_style, 0);
  }

  // Status section allocations.
  status_column = lv_obj_create(screen);
  lv_obj_set_size(status_column, TFT_WIDTH, 30);
  lv_obj_set_scrollbar_mode(status_column, LV_SCROLLBAR_MODE_OFF);
  lv_obj_align(status_column, LV_ALIGN_BOTTOM_MID, 0, 5);
  lv_obj_add_style(status_column, &screen_style, 0);

  status_label = lv_label_create(status_column);
  lv_obj_add_style(status_label, &label_style, 0);
  lv_obj_align(status_label, LV_ALIGN_BOTTOM_LEFT, 0, 0);

  status_icon_label = lv_label_create(status_column);
  lv_obj_add_style(status_icon_label, &icon_label_style, 0);
  lv_obj_align(status_icon_label, LV_ALIGN_BOTTOM_RIGHT, 0, 0);

  //
  // lvgl resources ready.
  //

  log_i("lvgl ready.");

  if (vcnl.begin()) {
    log_d("vcnl proximity sensor detected!");
    prox_ready = true;
  } else {
    log_e("[warning] no vcnl proximity sensor detected!");
    failed = true;
  }

  log_d("boot complete, redis-config. host: %s | port: %d", redis_host, redis_port);
  eng.begin();

  digitalWrite(LCD_PIN_NUM_BCKL, HIGH);
}

void loop(void) {
  auto now = millis();

  uint16_t prox = prox_ready ? vcnl.readProximity() : 0;

#ifndef RELEASE
  bool print_debug_info = _debug_timer.update(now) == 1;
  if (print_debug_info) {
    log_d("proximity (enabled %d): %d", prox_ready, prox);
    log_d("free memory before update: %d (max %d)", ESP.getFreeHeap(), ESP.getMaxAllocHeap());
  }
#endif

  // Apply updates.
  state = eng.update(state, now);

  if (std::get_if<WorkingState>(&state.active)) {
    WorkingState * working_state = std::get_if<WorkingState>(&state.active);
    lv_label_set_text(status_icon_label, "F");

    lv_label_set_text(status_label, working_state->id_content);

    uint8_t i = 0;
    for (auto message = working_state->begin(); message != working_state->end(); message++) {
      if (message->content_size > 0 && i < label_count) {
        lv_label_set_text(message_labels[i], message->content);
        i += 1;
      }
    }
  } else {
    for (uint8_t i = 0; i < label_count; i++) {
      lv_label_set_text(message_labels[i], "");
    }

    lv_label_set_text(status_icon_label, "J");
    lv_label_set_text(status_label, "connecting...");
  }

  lv_scr_load(screen);

  // Update the lvgl internal timer
  lv_tick_inc(now - last_frame);
  lv_timer_handler();

  last_frame = now;
#ifndef RELEASE
  if (print_debug_info) {
    log_d("free memory after update: %d (max %d)", ESP.getFreeHeap(), ESP.getMaxAllocHeap());
  }
#endif
}

