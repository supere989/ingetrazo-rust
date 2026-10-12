/* SPDX-License-Identifier: GPL-3.0-or-later */
/* Narrow scanline/destination adapter; codec is pinned libjpeg-turbo. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <setjmp.h>
#include <jpeglib.h>
#include <jerror.h>
typedef int (*SheetWrite)(void *,const unsigned char *,size_t);
typedef struct {struct jpeg_error_mgr base;jmp_buf jump;char message[JMSG_LENGTH_MAX];} SheetError;
typedef struct {struct jpeg_destination_mgr base;unsigned char bytes[16384];SheetWrite write;void *opaque;} SheetDestination;
typedef struct {struct jpeg_compress_struct codec;SheetError error;SheetDestination dest;int created;} SheetJpeg;
static void sheet_error(j_common_ptr codec){SheetError *error=(SheetError *)codec->err;(*codec->err->format_message)(codec,error->message);longjmp(error->jump,1);}
static void sheet_init(j_compress_ptr codec){SheetDestination *dest=(SheetDestination *)codec->dest;dest->base.next_output_byte=dest->bytes;dest->base.free_in_buffer=sizeof(dest->bytes);}
static boolean sheet_empty(j_compress_ptr codec){SheetDestination *dest=(SheetDestination *)codec->dest;if(!dest->write(dest->opaque,dest->bytes,sizeof(dest->bytes)))ERREXIT(codec,JERR_FILE_WRITE);sheet_init(codec);return TRUE;}
static void sheet_term(j_compress_ptr codec){SheetDestination *dest=(SheetDestination *)codec->dest;size_t bytes=sizeof(dest->bytes)-dest->base.free_in_buffer;if(bytes&&!dest->write(dest->opaque,dest->bytes,bytes))ERREXIT(codec,JERR_FILE_WRITE);}
void *ingetrazo_sheet_jpeg_new(unsigned width,unsigned height,unsigned density_unit,unsigned density_x,unsigned density_y,SheetWrite write,void *opaque,char *error,size_t error_size){
 SheetJpeg *out=(SheetJpeg *)calloc(1,sizeof(*out));if(!out){snprintf(error,error_size,"JPEG state allocation failed");return NULL;}
 out->codec.err=jpeg_std_error(&out->error.base);out->error.base.error_exit=sheet_error;
 if(setjmp(out->error.jump)){snprintf(error,error_size,"%s",out->error.message);if(out->created)jpeg_destroy_compress(&out->codec);free(out);return NULL;}
 out->created=1;jpeg_create_compress(&out->codec);out->codec.image_width=width;out->codec.image_height=height;out->codec.input_components=3;out->codec.in_color_space=JCS_RGB;
 jpeg_set_defaults(&out->codec);out->codec.density_unit=density_unit;out->codec.X_density=density_x;out->codec.Y_density=density_y;
 jpeg_set_quality(&out->codec,92,TRUE);for(int i=0;i<out->codec.num_components;i++){out->codec.comp_info[i].h_samp_factor=1;out->codec.comp_info[i].v_samp_factor=1;}
 out->dest.base.init_destination=sheet_init;out->dest.base.empty_output_buffer=sheet_empty;out->dest.base.term_destination=sheet_term;out->dest.write=write;out->dest.opaque=opaque;out->codec.dest=&out->dest.base;
 jpeg_start_compress(&out->codec,TRUE);return out;
}
int ingetrazo_sheet_jpeg_row(void *state,const unsigned char *pixels,char *error,size_t error_size){SheetJpeg *out=(SheetJpeg *)state;if(setjmp(out->error.jump)){snprintf(error,error_size,"%s",out->error.message);return 0;}JSAMPROW row=(JSAMPROW)pixels;return jpeg_write_scanlines(&out->codec,&row,1)==1;}
int ingetrazo_sheet_jpeg_finish(void *state,char *error,size_t error_size){SheetJpeg *out=(SheetJpeg *)state;int ok=1;if(setjmp(out->error.jump)){snprintf(error,error_size,"%s",out->error.message);ok=0;}else{jpeg_finish_compress(&out->codec);}jpeg_destroy_compress(&out->codec);free(out);return ok;}
void ingetrazo_sheet_jpeg_abort(void *state){SheetJpeg *out=(SheetJpeg *)state;if(out){jpeg_destroy_compress(&out->codec);free(out);}}
