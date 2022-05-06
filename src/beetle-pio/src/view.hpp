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
      gfx::ssize16 dims = (gfx::ssize16) _lcd.dimensions();

      const gfx::open_font & text_font = TEXT_FONT;
      const gfx::open_font & icon_font = ICON_FONT;
      float text_scale = text_font.scale(30);
      float icon_scale = icon_font.scale(30);

      gfx::size16 header_size(240, 30);
      uint8_t * header_buf = (uint8_t*) malloc(bmp_type::sizeof_buffer(header_size));
      bmp_type header_bitmap(header_size, header_buf);

      if (const ConfiguringState * conf = std::get_if<ConfiguringState>(&state.active)) {
        gfx::srect16 tr = text_font.measure_text(dims, {0, 0}, CONFIGURING, text_scale).bounds();
        gfx::srect16 ir = icon_font.measure_text(dims, {0, 0}, "A", icon_scale).bounds();

        gfx::draw::filled_rectangle(header_bitmap, (gfx::srect16) bnds, lcd_color::black);

        gfx::draw::text(
          header_bitmap,
          ir.offset(0, 0),
          {0, 0},
          "A",
          icon_font,
          icon_scale,
          lcd_color::red,
          lcd_color::black,
          false
        );
        gfx::draw::text(
          header_bitmap,
          tr.offset(36, 0),
          {0, 0},
          CONFIGURING,
          text_font,
          text_scale,
          lcd_color::white,
          lcd_color::black,
          false
        );
      } else if (const ConnectingState * con = std::get_if<ConnectingState>(&state.active)) {
        char * buffer = (char *) calloc(50, sizeof(char));
        sprintf(buffer, "connecting (%d)", con->attempt);
        gfx::srect16 tr = text_font.measure_text((gfx::ssize16) dims, {0, 0}, buffer, text_scale).bounds();
        gfx::draw::filled_rectangle(header_bitmap, (gfx::srect16) bnds, lcd_color::black);
        gfx::draw::text(
          header_bitmap,
          tr.offset(0, 0),
          {0, 0},
          buffer,
          text_font,
          text_scale,
          lcd_color::white,
          lcd_color::black,
          false
        );
        free(buffer);
      } else if (const ConnectedState * con = std::get_if<ConnectedState>(&state.active)) {
        gfx::srect16 tr = text_font.measure_text((gfx::ssize16) dims, {0, 0}, "connected", text_scale).bounds();
        gfx::draw::filled_rectangle(header_bitmap, (gfx::srect16) bnds, lcd_color::black);
        gfx::draw::text(
          header_bitmap,
          tr.offset(0, 0),
          {0, 0},
          "connected",
          text_font,
          text_scale,
          lcd_color::white,
          lcd_color::black,
          false
        );
      }

      gfx::draw::bitmap(_lcd, (gfx::srect16) bnds, header_bitmap, header_bitmap.bounds());
      free(header_buf);

      // draw footer
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
