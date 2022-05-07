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

template<class T>
static void icon_line(T lcd, char icon, const char * message) {
  using lcd_color = gfx::color<typename T::pixel_type>;
  using bmp_type = gfx::bitmap<typename T::pixel_type>;

  auto bnds = lcd.bounds();
  gfx::ssize16 dims = (gfx::ssize16) lcd.dimensions();

  auto fg = lcd_color::white;
  auto bg = lcd_color::black;

  const gfx::open_font & text_font = TEXT_FONT;
  const gfx::open_font & icon_font = ICON_FONT;
  float text_scale = text_font.scale(30);
  float icon_scale = icon_font.scale(30);

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
  gfx::draw::text(bitmap, rect.offset(30, 0), {0, 0}, message, text_font, text_scale, fg, bg, false);

  // Finish by drawing the bitmap into the actual screen.
  gfx::draw::bitmap(lcd, (gfx::srect16) bnds, bitmap, bitmap.bounds());

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

    void flip_screen(typename T::EScreenOrientation orientation) {
      _lcd.rotate(orientation);
    }

    void render(const State& state) {
      auto bnds = _lcd.bounds();
      gfx::ssize16 dims = (gfx::ssize16) _lcd.dimensions();

      const gfx::open_font & icon_font = ICON_FONT;
      float icon_scale = icon_font.scale(30);

      if (const ConfiguringState * conf = std::get_if<ConfiguringState>(&state.active)) {
        icon_line(_lcd, 'F', "configuring");
      } else if (const ConnectingState * con = std::get_if<ConnectingState>(&state.active)) {
        icon_line(_lcd, 'E', "connecting");
      } else if (const ConnectedState * con = std::get_if<ConnectedState>(&state.active)) {
        icon_line(_lcd, 'I', "connected");
      } else if (const WorkingState * work = std::get_if<WorkingState>(&state.active)) {
        bool has_message = work->message_size > 0;
        icon_line(_lcd, has_message ? 'A' : 'B', has_message ? work->message_content : "working");
      }

      // draw footer
      gfx::size16 header_size(240, 30);
      uint8_t * icon_buf = (uint8_t*) malloc(bmp_type::sizeof_buffer(header_size));
      bmp_type icon_bmp(header_size, icon_buf);
      gfx::draw::filled_rectangle(icon_bmp, (gfx::srect16) bnds, lcd_color::black);
      gfx::srect16 text_rect = icon_font.measure_text((gfx::ssize16) dims, {0, 0}, "ABCDEF", icon_scale).bounds();
      gfx::draw::text(icon_bmp, text_rect, {0, 0}, "ABCDEF", icon_font, icon_scale, lcd_color::white, lcd_color::black, false);
      gfx::draw::bitmap(_lcd, (gfx::srect16) bnds.offset(0, bnds.height() - text_rect.height()), icon_bmp, icon_bmp.bounds().offset(0, 0));
      free(icon_buf);
    }

  private:
    T _lcd;
};

#endif
