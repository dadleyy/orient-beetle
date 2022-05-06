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
      auto bnds = _lcd.bounds();

      const gfx::open_font & f = TEXT_FONT;
      float ss = f.scale(30);

      gfx::size16 bmp_size(240, 30);
      uint8_t * line_buf = (uint8_t*) malloc(bmp_type::sizeof_buffer(bmp_size));
      bmp_type line_bmp(bmp_size, line_buf);

      if (const ConfiguringState * conf = std::get_if<ConfiguringState>(&state.active)) {
        gfx::srect16 tr = f.measure_text((gfx::ssize16) _lcd.dimensions(), {0, 0}, "conf", ss).bounds();
        gfx::draw::filled_rectangle(line_bmp, (gfx::srect16) bnds, lcd_color::black);
        gfx::draw::text(line_bmp, tr.offset(0, 0), {0, 0}, "conf", f, ss, lcd_color::white, lcd_color::black, false);
      } else if (const ConnectingState * con = std::get_if<ConnectingState>(&state.active)) {
        gfx::srect16 tr = f.measure_text((gfx::ssize16) _lcd.dimensions(), {0, 0}, "connecting", ss).bounds();
        gfx::draw::filled_rectangle(line_bmp, (gfx::srect16) bnds, lcd_color::black);
        gfx::draw::text(line_bmp, tr.offset(0, 0), {0, 0}, "connecting", f, ss, lcd_color::white, lcd_color::black, false);
      } else if (const ConnectedState * con = std::get_if<ConnectedState>(&state.active)) {
        gfx::srect16 tr = f.measure_text((gfx::ssize16) _lcd.dimensions(), {0, 0}, "connected", ss).bounds();
        gfx::draw::filled_rectangle(line_bmp, (gfx::srect16) bnds, lcd_color::black);
        gfx::draw::text(line_bmp, tr.offset(0, 0), {0, 0}, "connected", f, ss, lcd_color::white, lcd_color::black, false);
      }

      gfx::draw::bitmap(_lcd, (gfx::srect16) bnds, line_bmp, line_bmp.bounds());
      free(line_buf);

      // draw footer
      const gfx::open_font & gly = ICON_FONT;
      float scale = gly.scale(20);
      uint8_t * icon_buf = (uint8_t*) malloc(bmp_type::sizeof_buffer(bmp_size));
      bmp_type icon_bmp(bmp_size, icon_buf);
      gfx::draw::filled_rectangle(icon_bmp, (gfx::srect16) bnds, lcd_color::black);
      gfx::srect16 text_rect = gly.measure_text((gfx::ssize16) _lcd.dimensions(), {0, 0}, "ABCDEFGHIJK", scale).bounds();
      gfx::draw::text(icon_bmp, text_rect, {0, 0}, "ABCDEFGHIJK", gly, scale, lcd_color::white, lcd_color::black, false);
      gfx::draw::bitmap(_lcd, (gfx::srect16) bnds.offset(0, bnds.height() - text_rect.height()), icon_bmp, icon_bmp.bounds().offset(0, 0));
      free(icon_buf);
    }

  private:
    T _lcd;
};

#endif
