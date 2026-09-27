#include "lvgl.h"
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

typedef struct { int32_t x,y,w,h; uint32_t color; } lc_rect;
typedef struct { int32_t x,y; uint32_t color; char text[64]; } lc_text;
static lv_font_t font;
static lv_disp_t *display;
static lv_obj_t *root, *rects[128], *texts[8];
static lv_color_t *pixels;
static lv_disp_draw_buf_t draw_buffer;
static lv_disp_drv_t driver;
static unsigned width, height;
static uint16_t u16(const uint8_t *p) { return p[0] | p[1]<<8; }
static uint32_t u32(const uint8_t *p) { return u16(p) | (uint32_t)u16(p+2)<<16; }

/* This embedded resource is fixed at build time, not an external font parser.
 * Its 32-bit relative offsets are expanded to native pointers on all hosts. */
int lc_font_init(const uint8_t *data, size_t size) {
    if (font.dsc) return 1;
    if (size < 64) return 0;
    lv_init();
    uint32_t off = u32(data+24);
    if (off != 36 || size < off+20) return 0;
    const uint8_t *d = data+off;
    uint32_t glyph_off=u32(d+4), cmap_off=u32(d+8);
    unsigned flags=u16(d+18), cmap_count=flags&511;
    if (cmap_off<=glyph_off || off+cmap_off+cmap_count*20>size) return 0;
    unsigned glyph_count=(cmap_off-glyph_off)/16;
    lv_font_fmt_txt_dsc_t *fd=calloc(1,sizeof(*fd));
    lv_font_fmt_txt_glyph_dsc_t *gd=calloc(glyph_count,sizeof(*gd));
    lv_font_fmt_txt_cmap_t *maps=calloc(cmap_count,sizeof(*maps));
    lv_font_fmt_txt_kern_pair_t *kern=calloc(1,sizeof(*kern));
    if (!fd || !gd || !maps || !kern) { free(fd);free(gd);free(maps);free(kern);return 0; }
    for (unsigned i=0;i<glyph_count;i++) {
        const uint8_t *p=d+glyph_off+i*16;
        gd[i].bitmap_index=u32(p); gd[i].adv_w=u32(p+4);
        gd[i].box_w=u16(p+8); gd[i].box_h=u16(p+10);
        gd[i].ofs_x=(int16_t)u16(p+12); gd[i].ofs_y=(int16_t)u16(p+14);
    }
    for (unsigned i=0;i<cmap_count;i++) {
        const uint8_t *p=d+cmap_off+i*20;
        maps[i].range_start=u32(p);maps[i].range_length=u16(p+4);
        maps[i].glyph_id_start=u16(p+6);maps[i].list_length=u16(p+16);
        maps[i].type=p[18];
        maps[i].unicode_list=u32(p+8)?(const uint16_t *)(d+cmap_off+u32(p+8)):NULL;
        maps[i].glyph_id_ofs_list=u32(p+12)?d+cmap_off+u32(p+12):NULL;
    }
    const uint8_t *kp=d+u32(d+12);
    kern->glyph_ids=kp+u32(kp);kern->values=(const int8_t *)(kp+u32(kp+4));
    kern->pair_cnt=u32(kp+8)&0x3fffffff;kern->glyph_ids_size=u32(kp+8)>>30;
    fd->glyph_bitmap=d+u32(d);fd->glyph_dsc=gd;fd->cmaps=maps;
    fd->kern_dsc=kern;fd->kern_scale=u16(d+16);fd->cmap_num=cmap_count;
    fd->bpp=(flags>>9)&15;fd->kern_classes=(flags>>13)&1;fd->bitmap_format=flags>>14;
    font.get_glyph_dsc=lv_font_get_glyph_dsc_fmt_txt;
    font.get_glyph_bitmap=lv_font_get_bitmap_fmt_txt;
    font.line_height=18;font.base_line=0;font.underline_position=-2;
    font.underline_thickness=1;font.dsc=fd;
    return 1;
}
static void flush(lv_disp_drv_t *drv,const lv_area_t *area,lv_color_t *colors) {
    (void)area;(void)colors;lv_disp_flush_ready(drv);
}
static int prepare(unsigned w,unsigned h) {
    if (display && width==w && height==h) return 1;
    if (display) { lv_disp_remove(display); display=NULL;free(pixels);pixels=NULL; }
    width=w;height=h;pixels=calloc(w*h,sizeof(*pixels));if (!pixels) return 0;
    lv_disp_draw_buf_init(&draw_buffer,pixels,NULL,w*h);
    lv_disp_drv_init(&driver);driver.hor_res=w;driver.ver_res=h;
    driver.draw_buf=&draw_buffer;driver.flush_cb=flush;driver.full_refresh=1;
    display=lv_disp_drv_register(&driver);if (!display) return 0;
    root=lv_obj_create(lv_disp_get_scr_act(display));
    lv_obj_clear_flag(root,LV_OBJ_FLAG_SCROLLABLE);
    lv_obj_set_pos(root,0,0);lv_obj_set_size(root,w,h);
    lv_obj_set_style_pad_all(root,0,0);lv_obj_set_style_border_width(root,0,0);
    lv_obj_set_style_radius(root,0,0);lv_obj_set_style_bg_opa(root,LV_OPA_COVER,0);
    for (unsigned i=0;i<128;i++) {
        rects[i]=lv_obj_create(root);lv_obj_clear_flag(rects[i],LV_OBJ_FLAG_SCROLLABLE);
        lv_obj_set_style_border_width(rects[i],0,0);lv_obj_set_style_radius(rects[i],0,0);
        lv_obj_set_style_pad_all(rects[i],0,0);
    }
    for (unsigned i=0;i<8;i++) {
        texts[i]=lv_label_create(root);lv_obj_set_style_text_font(texts[i],&font,0);
        lv_obj_set_style_text_align(texts[i],LV_TEXT_ALIGN_LEFT,0);
    }
    return 1;
}
int lc_render(unsigned w,unsigned h,uint32_t bg,const lc_rect *r,unsigned nr,
              const lc_text *t,unsigned nt,uint8_t *rgba) {
    if (!font.dsc || !w || !h || w>2048 || h>2048 || nr>128 || nt>8 || !prepare(w,h)) return 0;
    lv_obj_set_style_bg_color(root,lv_color_hex(bg),0);
    for (unsigned i=0;i<128;i++) {
        if (i>=nr) {lv_obj_add_flag(rects[i],LV_OBJ_FLAG_HIDDEN);continue;}
        lv_obj_set_pos(rects[i],r[i].x,r[i].y);lv_obj_set_size(rects[i],r[i].w,r[i].h);
        lv_obj_set_style_bg_color(rects[i],lv_color_hex(r[i].color),0);
        lv_obj_set_style_bg_opa(rects[i],LV_OPA_COVER,0);
        lv_obj_clear_flag(rects[i],LV_OBJ_FLAG_HIDDEN);
    }
    for (unsigned i=0;i<8;i++) {
        if (i>=nt) {lv_obj_add_flag(texts[i],LV_OBJ_FLAG_HIDDEN);continue;}
        lv_obj_set_pos(texts[i],t[i].x,t[i].y);
        lv_obj_set_style_text_color(texts[i],lv_color_hex(t[i].color),0);
        lv_label_set_text(texts[i],t[i].text);lv_obj_clear_flag(texts[i],LV_OBJ_FLAG_HIDDEN);
    }
    lv_obj_invalidate(root);lv_refr_now(display);
    for (unsigned i=0;i<w*h;i++) {
        lv_color32_t c;c.full=lv_color_to32(pixels[i]);
        rgba[i*4]=c.ch.red;rgba[i*4+1]=c.ch.green;rgba[i*4+2]=c.ch.blue;rgba[i*4+3]=255;
    }
    return 1;
}
