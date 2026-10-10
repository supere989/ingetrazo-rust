/*
 * Copyright © 2018  Ebrahim Byagowi
 * Copyright © 2018  Google, Inc.
 *
 *  This is part of HarfBuzz, a text shaping library.
 *
 * Permission is hereby granted, without written agreement and without
 * license or royalty fees, to use, copy, modify, and distribute this
 * software and its documentation for any purpose, provided that the
 * above copyright notice and the following two paragraphs appear in
 * all copies of this software.
 *
 * IN NO EVENT SHALL THE COPYRIGHT HOLDER BE LIABLE TO ANY PARTY FOR
 * DIRECT, INDIRECT, SPECIAL, INCIDENTAL, OR CONSEQUENTIAL DAMAGES
 * ARISING OUT OF THE USE OF THIS SOFTWARE AND ITS DOCUMENTATION, EVEN
 * IF THE COPYRIGHT HOLDER HAS BEEN ADVISED OF THE POSSIBILITY OF SUCH
 * DAMAGE.
 *
 * THE COPYRIGHT HOLDER SPECIFICALLY DISCLAIMS ANY WARRANTIES, INCLUDING,
 * BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND
 * FITNESS FOR A PARTICULAR PURPOSE.  THE SOFTWARE PROVIDED HEREUNDER IS
 * ON AN "AS IS" BASIS, AND THE COPYRIGHT HOLDER HAS NO OBLIGATION TO
 * PROVIDE MAINTENANCE, SUPPORT, UPDATES, ENHANCEMENTS, OR MODIFICATIONS.
 *
 * Google Author(s): Behdad Esfahbod
 */

#ifndef HB_AAT_LAYOUT_TRAK_TABLE_HH
#define HB_AAT_LAYOUT_TRAK_TABLE_HH

#include "hb-aat-layout-common.hh"
#include "hb-ot-layout.hh"
#include "hb-open-type.hh"

/*
 * trak -- Tracking
 * https://developer.apple.com/fonts/TrueType-Reference-Manual/RM06/Chap6trak.html
 */
#define HB_AAT_TAG_trak HB_TAG('t','r','a','k')


namespace AAT {

// Explicit native-paper policy. General fonts retain HarfBuzz 8.4 behavior.
static hb_user_data_key_t ingetrazo_qt_tracking_key;


struct TrackTableEntry
{
  friend struct TrackData;

  float get_track_value () const { return track.to_float (); }

  int get_value (const void *base, unsigned int index,
		 unsigned int table_size) const
  { return (base+valuesZ).as_array (table_size)[index]; }

  float interpolate_at (unsigned int idx,
			float ptem,
			const void *base,
			hb_array_t<const F16DOT16> size_table) const
  {
    const FWORD *values = (base+valuesZ).arrayZ;

    float s0 = size_table[idx].to_float ();
    float s1 = size_table[idx + 1].to_float ();
    int v0 = values[idx];
    int v1 = values[idx + 1];

    // Deal with font bugs.
    if (unlikely (s1 < s0))
    { hb_swap (s0, s1); hb_swap (v0, v1); }
    if (unlikely (ptem < s0)) return v0;
    if (unlikely (ptem > s1)) return v1;
    if (unlikely (s0 == s1)) return (v0 + v1) * 0.5f;

    float t = (ptem - s0) / (s1 - s0);
    return v0 + t * (v1 - v0);
  }

  float get_qt_value (float ptem,
		   const void *base,
		   hb_array_t<const F16DOT16> size_table) const
  {
    const FWORD *values = (base+valuesZ).arrayZ;

    unsigned int n_sizes = size_table.length;

    /*
     * Choose size.
     */
    if (!n_sizes) return 0.f;
    if (n_sizes == 1) return values[0];

    // At least two entries.

    unsigned i;
    for (i = 0; i < n_sizes; i++)
      if (size_table[i].to_float () >= ptem)
	break;

    // Boundary conditions.
    if (i == 0)       return values[0];
    if (i == n_sizes) return values[n_sizes - 1];

    // Exact match.
    if (size_table[i].to_float () == ptem) return values[i];

    // Interpolate.
    return interpolate_at (i - 1, ptem, base, size_table);
  }

  public:
  bool sanitize (hb_sanitize_context_t *c, const void *base,
		 unsigned int table_size) const
  {
    TRACE_SANITIZE (this);
    return_trace (likely (c->check_struct (this) &&
			  (valuesZ.sanitize (c, base, table_size))));
  }

  protected:
  F16DOT16	track;		/* Track value for this record. */
  NameID	trackNameID;	/* The 'name' table index for this track.
				 * (a short word or phrase like "loose"
				 * or "very tight") */
  NNOffset16To<UnsizedArrayOf<FWORD>>
		valuesZ;	/* Offset from start of tracking table to
				 * per-size tracking values for this track. */

  public:
  DEFINE_SIZE_STATIC (8);
};

struct TrackData
{
  float interpolate_at (unsigned int idx,
			float target_size,
			const TrackTableEntry &trackTableEntry,
			const void *base) const
  {
    unsigned int sizes = nSizes;
    hb_array_t<const F16DOT16> size_table ((base+sizeTable).arrayZ, sizes);

    float s0 = size_table[idx].to_float ();
    float s1 = size_table[idx + 1].to_float ();
    float t = unlikely (s0 == s1) ? 0.f : (target_size - s0) / (s1 - s0);
    return t * trackTableEntry.get_value (base, idx + 1, sizes) +
	   (1.f - t) * trackTableEntry.get_value (base, idx, sizes);
  }

  int get_tracking (const void *base, float ptem) const
  {
    /*
     * Choose track.
     */
    const TrackTableEntry *trackTableEntry = nullptr;
    unsigned int count = nTracks;
    for (unsigned int i = 0; i < count; i++)
    {
      /* Note: Seems like the track entries are sorted by values.  But the
       * spec doesn't explicitly say that.  It just mentions it in the example. */

      /* For now we only seek for track entries with zero tracking value */

      if (trackTable[i].get_track_value () == 0.f)
      {
	trackTableEntry = &trackTable[i];
	break;
      }
    }
    if (!trackTableEntry) return 0;

    /*
     * Choose size.
     */
    unsigned int sizes = nSizes;
    if (!sizes) return 0;
    if (sizes == 1) return trackTableEntry->get_value (base, 0, sizes);

    hb_array_t<const F16DOT16> size_table ((base+sizeTable).arrayZ, sizes);
    unsigned int size_index;
    for (size_index = 0; size_index < sizes - 1; size_index++)
      if (size_table[size_index].to_float () >= ptem)
	break;

    return roundf (interpolate_at (size_index ? size_index - 1 : 0, ptem,
				   *trackTableEntry, base));
  }

  float get_qt_tracking (const void *base, float ptem, float track = 0.f) const
  {
    unsigned count = nTracks;
    hb_array_t<const F16DOT16> size_table = (base+sizeTable).as_array (nSizes);

    if (!count) return 0.f;
    if (count == 1) return trackTable[0].get_qt_value (ptem, base, size_table);

    // At least two entries.

    unsigned i = 0;
    unsigned j = count - 1;

    // Find the two entries that track is between.
    while (i + 1 < count && trackTable[i + 1].get_track_value () <= track)
      i++;
    while (j > 0 && trackTable[j - 1].get_track_value () >= track)
      j--;

    // Exact match.
    if (i == j) return trackTable[i].get_qt_value (ptem, base, size_table);

    // Interpolate.

    float t0 = trackTable[i].get_track_value ();
    float t1 = trackTable[j].get_track_value ();

    float t = (track - t0) / (t1 - t0);

    float a = trackTable[i].get_qt_value (ptem, base, size_table);
    float b = trackTable[j].get_qt_value (ptem, base, size_table);
    return a + t * (b - a);
  }

  bool sanitize (hb_sanitize_context_t *c, const void *base) const
  {
    TRACE_SANITIZE (this);
    return_trace (likely (c->check_struct (this) &&
			  hb_barrier () &&
			  sizeTable.sanitize (c, base, nSizes) &&
			  trackTable.sanitize (c, nTracks, base, nSizes)));
  }

  protected:
  HBUINT16	nTracks;	/* Number of separate tracks included in this table. */
  HBUINT16	nSizes;		/* Number of point sizes included in this table. */
  NNOffset32To<UnsizedArrayOf<F16DOT16>>
		sizeTable;	/* Offset from start of the tracking table to
				 * Array[nSizes] of size values.. */
  UnsizedArrayOf<TrackTableEntry>
		trackTable;	/* Array[nTracks] of TrackTableEntry records. */

  public:
  DEFINE_SIZE_ARRAY (8, trackTable);
};

struct trak
{
  static constexpr hb_tag_t tableTag = HB_AAT_TAG_trak;

  bool has_data () const { return version.to_int (); }

  bool apply (hb_aat_apply_context_t *c) const
  {
    TRACE_APPLY (this);

    hb_mask_t trak_mask = c->plan->trak_mask;

    const float ptem = c->font->ptem;
    if (unlikely (ptem <= 0.f))
      return_trace (false);

    hb_buffer_t *buffer = c->buffer;
    if (hb_font_get_user_data (c->font, &ingetrazo_qt_tracking_key))
    {
      // Qt 6.12 / HarfBuzz 14.5: apply tracking to grapheme advances only.
      // Keep HarfBuzz's internal continuation flags, not output cluster guesses.
      const bool horizontal = HB_DIRECTION_IS_HORIZONTAL (buffer->props.direction);
      const TrackData &data = horizontal ? this+horizData : this+vertData;
      const float tracking = data.get_qt_tracking (this, ptem);
      const hb_position_t delta = horizontal ? c->font->em_scalef_x (tracking)
                                            : c->font->em_scalef_y (tracking);
      foreach_grapheme (buffer, start, end)
      {
        hb_position_t &advance = horizontal ? buffer->pos[start].x_advance
                                           : buffer->pos[start].y_advance;
        // Modern HarfBuzz saturates additions; do not allow signed overflow.
        const int64_t sum = (int64_t) advance + delta;
        advance = sum > INT32_MAX ? INT32_MAX : sum < INT32_MIN ? INT32_MIN : sum;
      }
      return_trace (true);
    }
    if (HB_DIRECTION_IS_HORIZONTAL (buffer->props.direction))
    {
      const TrackData &trackData = this+horizData;
      int tracking = trackData.get_tracking (this, ptem);
      hb_position_t offset_to_add = c->font->em_scalef_x (tracking / 2);
      hb_position_t advance_to_add = c->font->em_scalef_x (tracking);
      foreach_grapheme (buffer, start, end)
      {
	if (!(buffer->info[start].mask & trak_mask)) continue;
	buffer->pos[start].x_advance += advance_to_add;
	buffer->pos[start].x_offset += offset_to_add;
      }
    }
    else
    {
      const TrackData &trackData = this+vertData;
      int tracking = trackData.get_tracking (this, ptem);
      hb_position_t offset_to_add = c->font->em_scalef_y (tracking / 2);
      hb_position_t advance_to_add = c->font->em_scalef_y (tracking);
      foreach_grapheme (buffer, start, end)
      {
	if (!(buffer->info[start].mask & trak_mask)) continue;
	buffer->pos[start].y_advance += advance_to_add;
	buffer->pos[start].y_offset += offset_to_add;
      }
    }

    return_trace (true);
  }

  bool sanitize (hb_sanitize_context_t *c) const
  {
    TRACE_SANITIZE (this);

    return_trace (likely (c->check_struct (this) &&
			  hb_barrier () &&
			  version.major == 1 &&
			  horizData.sanitize (c, this, this) &&
			  vertData.sanitize (c, this, this)));
  }

  protected:
  FixedVersion<>version;	/* Version of the tracking table
				 * (0x00010000u for version 1.0). */
  HBUINT16	format;		/* Format of the tracking table (set to 0). */
  Offset16To<TrackData>
		horizData;	/* Offset from start of tracking table to TrackData
				 * for horizontal text (or 0 if none). */
  Offset16To<TrackData>
		vertData;	/* Offset from start of tracking table to TrackData
				 * for vertical text (or 0 if none). */
  HBUINT16	reserved;	/* Reserved. Set to 0. */

  public:
  DEFINE_SIZE_STATIC (12);
};

} /* namespace AAT */


#endif /* HB_AAT_LAYOUT_TRAK_TABLE_HH */
