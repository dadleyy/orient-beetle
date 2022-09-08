#ifndef _VIEW_H
#define _VIEW_H

#include "jellee_ttf.hpp"
#include "glyphter_ttf.hpp"
#include "state.hpp"

// TODO: the header files were generated from this directory using:
//
// ```
// fontgen ../../.resources/Glyphter-font/Glyphter.ttf > ./include/glyphter_ttf.hpp
// fontgen ../../.resources/Jellee_1223/TTF/Jellee-Bold.ttf > ./include/jellee_ttf.hpp
// ```
//
// The generated symbols aren't particularly nice, so we'll macro them away for now.
#define ICON_FONT _______resources_Glyphter_font_Glyphter_ttf
#define TEXT_FONT _______resources_Jellee_1223_TTF_Jellee_Bold_ttf

constexpr const char * CONFIGURING = "pending setup";

const char ICN_UP_ARROW = 'A';
const char ICN_RIGHT_ARROW = 'B';
const char ICN_DOWN_ARROW = 'C';
const char ICN_LEFT_ARROW = 'D';
const char ICN_INFO = 'F';
const char ICN_WIFI = 'J';
const char ICN_CHAT_BUBBLE = 'N';

template<class T>
static void icon_line(T lcd, char icon, const char * message, uint8_t position = 0) {
  using lcd_color = gfx::color<typename T::pixel_type>;
  using bmp_type = gfx::bitmap<typename T::pixel_type>;

  auto bnds = lcd.bounds();
  gfx::ssize16 dims = (gfx::ssize16) lcd.dimensions();

  auto fg = lcd_color::white;
  auto bg = lcd_color::black;

  const gfx::open_font & text_font = TEXT_FONT;
  const gfx::open_font & icon_font = ICON_FONT;
  float text_scale = position == 1 ? text_font.scale(20) : text_font.scale(30);
  float icon_scale = position == 1 ? icon_font.scale(20) : icon_font.scale(30);

  gfx::size16 bounds(240, 30);
  uint8_t * buffer = (uint8_t*) malloc(bmp_type::sizeof_buffer(bounds));

  // Prepare a bitmap draw target
  bmp_type bitmap(bounds, buffer);

  // Clear out the draw target (assumes same location)
  gfx::draw::filled_rectangle(bitmap, (gfx::srect16) bnds, bg);

  // Draw the icon into the bitmap.
  char is [2] = {icon, '\0'};
  gfx::srect16 icon_rect = icon_font.measure_text((gfx::ssize16) dims, {0, 0}, is, icon_scale).bounds();
  gfx::draw::text(bitmap, icon_rect, {0, 0}, is, icon_font, icon_scale, fg, bg, false);

  // Draw the message text
  gfx::srect16 rect = text_font.measure_text((gfx::ssize16) dims, {0, 0}, message, text_scale).bounds();
  gfx::draw::text(bitmap, rect.offset(38, 0), {0, 0}, message, text_font, text_scale, fg, bg, false);

  // Finish by drawing the bitmap into the actual screen.
  switch (position) {
    case 0:
      gfx::draw::bitmap(lcd, (gfx::srect16) bnds, bitmap, bitmap.bounds());
      break;
    case 1:
      gfx::draw::bitmap(lcd, (gfx::srect16) bnds.offset(0, bnds.height() - rect.height()), bitmap, bitmap.bounds());
      break;
    default:
      gfx::draw::bitmap(lcd, (gfx::srect16) bnds.offset(0, position), bitmap, bitmap.bounds());
      break;
  }

  // Free up our memory.
  free(buffer);
}

template<class T>
class View final {
  using lcd_color = gfx::color<typename T::pixel_type>;
  using bmp_type = gfx::bitmap<typename T::pixel_type>;

  public:
    View() = default;
    ~View() = default;

    void clear(void) {
      gfx::draw::filled_rectangle(_lcd, (gfx::srect16) _lcd.bounds(), lcd_color::black);
    }

    void render(const State& state) {
      if (!init) {
        init_driver();
      }

      auto bnds = _lcd.bounds();
      gfx::ssize16 dims = (gfx::ssize16) _lcd.dimensions();

      const gfx::open_font & icon_font = ICON_FONT;
      float icon_scale = icon_font.scale(30);

      if (const ConfiguringState * conf = std::get_if<ConfiguringState>(&state.active)) {
        rm_footer = true;
        icon_line(_lcd, ICN_INFO, "configuring");
      } else if (const ConnectingState * con = std::get_if<ConnectingState>(&state.active)) {
        rm_footer = true;
        icon_line(_lcd, ICN_WIFI, "connecting");
      } else if (const ConnectedState * con = std::get_if<ConnectedState>(&state.active)) {
        rm_footer = true;
        icon_line(_lcd, 'I', "connected");
      } else if (const WorkingState * work = std::get_if<WorkingState>(&state.active)) {
        bool has_message = false;

        uint8_t i = 0;
        for (auto start = work->begin(); start != work->end(); start++) {
          if (start->content_size > 0) {
            has_message = true;
            icon_line(_lcd, ICN_CHAT_BUBBLE, start->content, i * 40);
            i += 1;
          }
        }

        if (!has_message) {
          icon_line(_lcd, ICN_CHAT_BUBBLE, "working");
        }

        if (work->id_size > 0) {
          // draw footer
          if (rm_footer) {
            gfx::size16 header_size(240, 30);
            uint8_t * icon_buf = (uint8_t*) malloc(bmp_type::sizeof_buffer(header_size));
            bmp_type icon_bmp(header_size, icon_buf);
            gfx::draw::filled_rectangle(icon_bmp, (gfx::srect16) bnds, lcd_color::black);
            gfx::srect16 text_rect = icon_font.measure_text((gfx::ssize16) dims, {0, 0}, "ABCDEF", icon_scale).bounds();
            gfx::draw::bitmap(_lcd, (gfx::srect16) bnds.offset(0, bnds.height() - text_rect.height()), icon_bmp, icon_bmp.bounds().offset(0, 0));
            free(icon_buf);
            rm_footer = false;
          }

          icon_line(_lcd, ICN_INFO, work->id_content, 1);
        }

        return;
      }

      rm_footer = true;
      // draw footer
      gfx::size16 header_size(240, 30);
      uint8_t * icon_buf = (uint8_t*) malloc(bmp_type::sizeof_buffer(header_size));
      bmp_type icon_bmp(header_size, icon_buf);
      gfx::draw::filled_rectangle(icon_bmp, (gfx::srect16) bnds, lcd_color::black);
      gfx::srect16 text_rect = icon_font.measure_text((gfx::ssize16) dims, {0, 0}, "ABCDEF", icon_scale).bounds();
      gfx::draw::text(icon_bmp, text_rect, {0, 0}, "ABCDEF", icon_font, icon_scale, lcd_color::red, lcd_color::black, false);
      gfx::draw::bitmap(_lcd, (gfx::srect16) bnds.offset(0, bnds.height() - text_rect.height()), icon_bmp, icon_bmp.bounds().offset(0, 0));
      free(icon_buf);
    }

  private:
    T _lcd;
    bool rm_footer;
    bool init = false;

    // TODO: we're still working on the oddities related to the ili9341v driver that is present in 
    // the orient-display that is being used.
    void init_driver(void) {
      init = true;
      T::bus::begin_write();
      T::bus::begin_transaction();
      T::driver::send_command(0x01);  // Command: Software Reset
      T::driver::send_command(0xCF);  // Command: Power Control.
      T::driver::send_data8(0x00);
      T::driver::send_data8(0xC1);
      T::driver::send_data8(0x30);
      T::driver::send_command(0xED);  // Command: Power On Sequence Control.
      T::driver::send_data8(0x64);
      T::driver::send_data8(0x03);
      T::driver::send_data8(0x12);
      T::driver::send_data8(0x81);
      T::driver::send_command(0xE8);  // Command: Driver Timing Control.
      T::driver::send_data8(0x85);
      T::driver::send_data8(0x00);
      T::driver::send_data8(0x78);
      T::driver::send_command(0xCB);  // Command: Power Control.
      T::driver::send_data8(0x39);
      T::driver::send_data8(0x2C);
      T::driver::send_data8(0x00);
      T::driver::send_data8(0x34);
      T::driver::send_data8(0x02);
      T::driver::send_command(0xF7);  // Command: Pump Ratio Control.
      T::driver::send_data8(0x20);
      T::driver::send_command(0xEA);  // Command: Driver Timing Control C.
      T::driver::send_data8(0x00);
      T::driver::send_data8(0x00);

      T::driver::send_command(0xB1);  // Command: Frame Rate Control.
      T::driver::send_data8(0x00);
      T::driver::send_data8(0x1B);

      T::driver::send_command(0xC0);  // Command: Power Control 1.
      T::driver::send_data8(0x21);
      T::driver::send_command(0xC1);  // Command: Power Control 2.
      T::driver::send_data8(0x11);

      T::driver::send_command(0xC5);  // Command: VCOM control 1.
      T::driver::send_data8(0x3e);
      T::driver::send_data8(0x28);
      T::driver::send_command(0xC7);  // Command: VCOM control 2.
      T::driver::send_data8(0x86);
      T::driver::send_command(0x36);  // Command: Memory Access Control.
      T::driver::send_data8(0x08);
      T::driver::send_command(0x3A);  // Command: COLMOD: Pixel Format Set.
      T::driver::send_data8(0x55);
      T::driver::send_command(0xB1);  // Command: Frame Rate Control.
      T::driver::send_data8(0x00);
      T::driver::send_data8(0x13);
      T::driver::send_command(0xB6);  // Command: Display Function control.
      T::driver::send_data8(0x08);
      T::driver::send_data8(0x82);
      T::driver::send_data8(0x27);
      T::driver::send_command(0xF2);  // Command: Enable 3G.
      T::driver::send_data8(0x00);
      T::driver::send_command(0x26);  // Command: Gamma Set
      T::driver::send_data8(0x01);

      T::driver::send_command(0xE0);  // Command: Pos Gamma Correction.
      T::driver::send_data8(0x0F);
      T::driver::send_data8(0x31);
      T::driver::send_data8(0x2B);
      T::driver::send_data8(0x0C);
      T::driver::send_data8(0x0E);
      T::driver::send_data8(0x08);
      T::driver::send_data8(0x4E);
      T::driver::send_data8(0xF1);
      T::driver::send_data8(0x37);
      T::driver::send_data8(0x07);
      T::driver::send_data8(0x10);
      T::driver::send_data8(0x03);
      T::driver::send_data8(0x0E);
      T::driver::send_data8(0x09);
      T::driver::send_data8(0x00);
      T::driver::send_command(0xE1);  // Command: Neg Gamma Correction.
      T::driver::send_data8(0x00);
      T::driver::send_data8(0x0E);
      T::driver::send_data8(0x14);
      T::driver::send_data8(0x03);
      T::driver::send_data8(0x11);
      T::driver::send_data8(0x07);
      T::driver::send_data8(0x31);
      T::driver::send_data8(0xC1);
      T::driver::send_data8(0x48);
      T::driver::send_data8(0x08);
      T::driver::send_data8(0x0F);
      T::driver::send_data8(0x0C);
      T::driver::send_data8(0x31);
      T::driver::send_data8(0x36);
      T::driver::send_data8(0x0F);
      T::driver::send_command(0x11);  // Command: Sleep OUT.
      T::bus::end_transaction();
      T::bus::end_write();

      delay(120);

      T::bus::begin_write();
      T::bus::begin_transaction();
      T::driver::send_command(0x29);  // Command: Display ON.
      T::bus::end_transaction();
      T::bus::end_write();

      T::bus::begin_write();
      T::bus::begin_transaction();
      T::driver::send_command(0x36);  // Command: Memory Access Control.
      T::driver::send_data8(0x00);
      T::driver::send_command(0xB6);  // Command: Display Function
      T::driver::send_data8(0x0A);
      T::driver::send_data8(0xA2);

      T::driver::send_command(0x11);  // Command: Sleep OUT
      T::bus::end_transaction();
      T::bus::end_write();

      delay(120);

      T::bus::begin_write();
      T::bus::begin_transaction();
      T::driver::send_command(0x29);  // Command: Display ON
      T::driver::send_command(0xb0);  // Command: RGB Interface control
      T::driver::send_data8(0x80);
      T::bus::end_transaction();
      T::bus::end_write();
    }
};

#endif
