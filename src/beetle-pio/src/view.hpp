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

    void render(const State& state) {
      gfx::size16 bmp_size(240, 30);
      uint8_t * line_buf = (uint8_t*) malloc(bmp_type::sizeof_buffer(bmp_size));
      bmp_type line_bmp(bmp_size, line_buf);
      gfx::draw::filled_rectangle(line_bmp, (gfx::srect16) _lcd.bounds(), lcd_color::black);
      gfx::draw::bitmap(_lcd, (gfx::srect16) _lcd.bounds(), line_bmp, line_bmp.bounds());
      free(line_buf);

      const gfx::open_font & gly = ICON_FONT;
      float scale = gly.scale(20);
      uint8_t * icon_buf = (uint8_t*) malloc(bmp_type::sizeof_buffer(bmp_size));
      bmp_type icon_bmp(bmp_size, icon_buf);
      gfx::draw::filled_rectangle(icon_bmp, (gfx::srect16) _lcd.bounds(), lcd_color::black);
      gfx::srect16 text_rect = gly.measure_text((gfx::ssize16) _lcd.dimensions(), {0, 0}, "ABCDEFGHIJK", scale).bounds();
      gfx::draw::text(icon_bmp, text_rect, {0, 0}, "ABCDEFGHIJK", gly, scale, lcd_color::white, lcd_color::black, false);
      gfx::draw::bitmap(_lcd, (gfx::srect16) _lcd.bounds().offset(0, 50), icon_bmp, icon_bmp.bounds().offset(0, 0));
      free(icon_buf);
    }

  private:
    T _lcd;
};

#endif
