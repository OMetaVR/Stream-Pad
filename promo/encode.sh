#!/usr/bin/env bash
# StreamPad promo encoder: PNG frame sequence -> README GIF + MP4 + WebM + poster + contact sheet.
#
#   promo/encode.sh [frames_dir=promo/build/frames] [out_dir=promo/out]
#
# Outputs (out_dir): streampad.gif  streampad.mp4  streampad.webm  poster.png  contact.png
#
# Frames must be a contiguous, zero-padded sequence (0000.png, 0001.png, ...), all the same size,
# captured at FPS (default 30). Everything is a pure function of the inputs: re-running overwrites.
#
# Tunables (environment variables, all optional):
#   GIF_WIDTH   960        output GIF width (lanczos downscale from the source; height keeps aspect)
#   GIF_FPS     = FPS      GIF frame rate; if lower than FPS the frames are resampled with ffmpeg's
#                          `fps=` filter (never skipped arbitrarily)
#   MAX_MB      12         GIF size budget in MB (10^6 bytes). The best-quality step is tried first; if it
#                          is over budget the script walks a ladder: fps -> GIF_MIN_FPS (default 25), then
#                          width -40 px per step down to GIF_MIN_WIDTH (default 720), then gifski's
#                          motion-/lossy-quality (see make_gif). Set GIF_MIN_FPS=GIF_FPS and
#                          GIF_MIN_WIDTH=GIF_WIDTH to forbid any reduction.
#   GIF_ENGINE  auto       auto = gifski when installed, else ffmpeg palettegen/paletteuse
#                          (gifski | ffmpeg to force)
#   FPS         30         frame rate of the input sequence
#   EXPECT_FRAMES 450      refuse to run if the frame count differs (catches a half-finished render); 0 = off
#   MP4_CRF 18  WEBM_CRF 27  MP4_WIDTH/WEBM_WIDTH 1280 (0 = source width)
#   POSTER_FRAME  frame number for poster.png (default: 327 = 10.9 s "Design every key"; middle if absent)
#   SKIP        space separated subset of: gif mp4 webm poster contact
#   NICE        10         `nice` level for every heavy command
#   MEM_KB      16000000   `ulimit -v` cap (KiB) for every heavy command
#
# Needs: bash, ffmpeg (libx264, libvpx-vp9), ffprobe; gifski strongly recommended (cargo install gifski).
set -euo pipefail

die()  { echo "encode.sh: error: $*" >&2; exit 1; }
info() { echo "encode.sh: $*" >&2; }

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
FRAMES=${1:-$ROOT/promo/build/frames}
OUT=${2:-$ROOT/promo/out}

FPS=${FPS:-30}
GIF_WIDTH=${GIF_WIDTH:-960}
GIF_FPS=${GIF_FPS:-$FPS}
GIF_MIN_FPS=${GIF_MIN_FPS:-25}
GIF_MIN_WIDTH=${GIF_MIN_WIDTH:-720}
MAX_MB=${MAX_MB:-12}
GIF_ENGINE=${GIF_ENGINE:-auto}
MP4_CRF=${MP4_CRF:-18}
WEBM_CRF=${WEBM_CRF:-27}
MP4_WIDTH=${MP4_WIDTH:-1280}
WEBM_WIDTH=${WEBM_WIDTH:-1280}
NICE=${NICE:-10}
MEM_KB=${MEM_KB:-16000000}
SKIP=${SKIP:-}
EXPECT_FRAMES=${EXPECT_FRAMES:-450}

export PATH="$HOME/.cargo/bin:$HOME/.local/bin:$PATH"

# ---- preflight --------------------------------------------------------------------------------
for t in ffmpeg ffprobe; do command -v "$t" >/dev/null || die "$t not found in PATH"; done
[ -d "$FRAMES" ] || die "frames dir not found: $FRAMES"

# A render may still be writing frames: wait (up to 4 min) until the newest frame is >5 s old.
for _ in $(seq 48); do
  newest=$(find "$FRAMES" -maxdepth 1 -name '*.png' -printf '%T@\n' | sort -n | tail -1)
  [ "$(awk -v n="$newest" -v t="$(date +%s.%N)" 'BEGIN{print (t-n > 5)}')" = 1 ] && break
  info "frames are still being written, waiting..."; sleep 5
done


shopt -s nullglob
files=("$FRAMES"/*.png)
shopt -u nullglob
N=${#files[@]}
[ "$N" -gt 0 ] || die "no .png files in $FRAMES"
{ [ "$EXPECT_FRAMES" -eq 0 ] || [ "$N" -eq "$EXPECT_FRAMES" ]; } || die "found $N frames, expected $EXPECT_FRAMES (render unfinished? set EXPECT_FRAMES=0 to override)"
first=$(basename "${files[0]}")
[[ $first =~ ^([0-9]+)\.png$ ]] || die "frame names must be zero-padded numbers like 0000.png (got $first)"
DIGITS=${#BASH_REMATCH[1]}
START=$((10#${BASH_REMATCH[1]}))
PATTERN="$FRAMES/%0${DIGITS}d.png"
# contiguity: last file must be START+N-1 (also catches holes from a half-finished render)
last=$(basename "${files[$((N - 1))]}" .png)
[ $((10#$last)) -eq $((START + N - 1)) ] || die "frame sequence has gaps: $N files, $first .. $last.png"

SRC_W=$(ffprobe -v error -select_streams v:0 -show_entries stream=width -of csv=p=0 "${files[0]}")
SRC_H=$(ffprobe -v error -select_streams v:0 -show_entries stream=height -of csv=p=0 "${files[0]}")
SECS=$(awk -v n="$N" -v f="$FPS" 'BEGIN{printf "%.2f", n/f}')
info "input: $N frames ${SRC_W}x${SRC_H} @ ${FPS} fps (${SECS} s) from $FRAMES"

mkdir -p "$OUT"
TMP=$(mktemp -d "${TMPDIR:-/tmp}/streampad-encode.XXXXXX")
trap 'rm -rf "$TMP"' EXIT
touch "$TMP/.start"   # any frame newer than this at the end => the sequence changed mid-encode

# Work from a private snapshot so a concurrent re-render cannot corrupt the encode.
SRC_DIR=$FRAMES
SNAP="$TMP/src"; mkdir -p "$SNAP"
cp -- "$SRC_DIR"/*.png "$SNAP"/ || die "could not snapshot frames from $SRC_DIR"
[ "$(find "$SNAP" -name '*.png' | wc -l)" -eq "$N" ] || die "frame count changed while snapshotting $SRC_DIR (render running?)"
FRAMES=$SNAP
PATTERN="$FRAMES/%0${DIGITS}d.png"

skip() { case " $SKIP " in *" $1 "*) return 0;; esac; return 1; }

# Run a heavy command niced and memory-capped (in a subshell, so the caller's limits are untouched).
heavy() { ( ulimit -v "$MEM_KB" 2>/dev/null || true; exec nice -n "$NICE" "$@" ); }

FF=(ffmpeg -hide_banner -loglevel error -nostdin -y)
IN=(-framerate "$FPS" -start_number "$START" -i "$PATTERN")

mb() { awk -v b="$(stat -c %s "$1")" 'BEGIN{printf "%.2f", b/1000000}'; }
# lanczos scale filter to an even width (0 => source width)
scale_for() {
  local w=$1
  [ "$w" -ne 0 ] || w=$SRC_W
  [ $((w % 2)) -eq 0 ] || w=$((w + 1))
  echo "scale=$w:-2:flags=lanczos+accurate_rnd+full_chroma_int"
}

# ---- GIF --------------------------------------------------------------------------------------
make_gif() {
  local engine=$GIF_ENGINE
  if [ "$engine" = auto ]; then
    if command -v gifski >/dev/null; then engine=gifski; else engine=ffmpeg; fi
  fi
  local tmpgif="$TMP/streampad.gif" vf
  vf=$(scale_for "$GIF_WIDTH")
  [ "$GIF_FPS" = "$FPS" ] || vf="fps=$GIF_FPS,$vf"

  if [ "$engine" = gifski ]; then
    command -v gifski >/dev/null || die "GIF_ENGINE=gifski but gifski is not installed (cargo install gifski)"
    # gifski does the colour work; it gets a clean lanczos-downscaled (and, if needed, fps-resampled via
    # ffmpeg's `fps=` filter) RGB PNG sequence. Temp PNGs are removed on exit.
    #
    # Quality/size ladder, tried in order until the file fits MAX_MB. Each step is "width:fps:Q:motion:lossy".
    # Measured on the real frames, Q=100/motion 100/lossy 100 is near lossless (no banding, no streaks,
    # no dither crawl) but busy content can cost > 12 MB at 960x30, so the cheapest *visual* sacrifices
    # come first: 30 -> GIF_MIN_FPS (25) fps, then narrower widths (-40 px steps, text stays crisp at the
    # ~830 px GitHub shows it at), and only then gifski's motion-quality / lossy knobs, which leave ghost
    # trails (motion) or grain on static gradients (lossy).
    local ladder=("$GIF_WIDTH:$GIF_FPS:100:100:100") w f=$GIF_FPS
    if [ "$f" -gt "$GIF_MIN_FPS" ]; then f=$GIF_MIN_FPS; ladder+=("$GIF_WIDTH:$f:100:100:100"); fi
    for ((w = GIF_WIDTH - 40; w >= GIF_MIN_WIDTH; w -= 40)); do ladder+=("$w:$f:100:100:100"); done
    local wmin=$(( w + 40 )); [ "$wmin" -lt "$GIF_WIDTH" ] || wmin=$GIF_WIDTH
    ladder+=("$wmin:$f:100:95:100" "$wmin:$f:100:90:100" "$wmin:$f:100:90:90")
    local max_bytes step q m l sw sf size=0 used="" prepkey=""
    max_bytes=$(awk -v m="$MAX_MB" 'BEGIN{printf "%d", m*1000000}')
    local pre="$TMP/gifsrc"
    for step in "${ladder[@]}"; do
      IFS=: read -r sw sf q m l <<<"$step"
      if [ "$prepkey" != "$sw:$sf" ]; then
        rm -rf "$pre"; mkdir -p "$pre"; prepkey="$sw:$sf"
        vf=$(scale_for "$sw"); [ "$sf" = "$FPS" ] || vf="fps=$sf,$vf"
        info "gif: scaling to ${sw}px wide @ ${sf} fps"
        heavy "${FF[@]}" "${IN[@]}" -vf "$vf" -pix_fmt rgb24 "$pre/%05d.png"
        local want=$(( (N * sf + FPS / 2) / FPS )) got
        got=$(find "$pre" -name '*.png' | wc -l)
        [ "$got" -ge $((want - 1)) ] && [ "$got" -le $((want + 1)) ] || die "scaled $got frames, expected ~$want (did the frames change while encoding?)"
      fi
      info "gif: gifski -r $sf -Q $q --motion-quality $m --lossy-quality $l  (${sw}px)"
      heavy gifski -r "$sf" -Q "$q" --motion-quality "$m" --lossy-quality "$l" \
        -o "$tmpgif" "$pre"/*.png >/dev/null 2>"$TMP/gifski.err" || { cat "$TMP/gifski.err" >&2; die "gifski failed"; }
      size=$(stat -c %s "$tmpgif"); used=$step
      [ "$size" -le "$max_bytes" ] && break
      info "gif: $(mb "$tmpgif") MB > ${MAX_MB} MB budget, trying the next step"
    done
    if [ "$size" -gt "$max_bytes" ]; then
      info "WARNING: GIF is $(mb "$tmpgif") MB, over the ${MAX_MB} MB budget even at the last ladder step ($used)"
    fi
    # The search above is fast (no --extra); polish the winning step with gifski's slower, ~2 % smaller mode.
    info "gif: polishing the chosen step with --extra"
    if heavy gifski --extra -r "$sf" -Q "$q" --motion-quality "$m" --lossy-quality "$l" \
         -o "$TMP/extra.gif" "$pre"/*.png >/dev/null 2>"$TMP/gifski.err" \
       && [ "$(stat -c %s "$TMP/extra.gif")" -lt "$size" ]; then
      mv -f "$TMP/extra.gif" "$tmpgif"
    fi
    info "gif: chosen width:fps:Q:motion:lossy = $used  ->  $(mb "$tmpgif") MB"
  else
    # ffmpeg fallback: stats_mode=diff palette + sierra2_4a error diffusion + rectangle diffing.
    # Noticeably larger (~1.6x) and less colour-accurate than gifski; install gifski for the real thing.
    info "gif: ffmpeg palettegen/paletteuse (sierra2_4a), ${GIF_WIDTH}px @ ${GIF_FPS} fps"
    heavy "${FF[@]}" "${IN[@]}" \
      -vf "$vf,split[a][b];[a]palettegen=stats_mode=diff:max_colors=256:reserve_transparent=0[p];[b][p]paletteuse=dither=sierra2_4a:diff_mode=rectangle" \
      -loop 0 "$tmpgif"
  fi
  mv -f "$tmpgif" "$OUT/streampad.gif"
}

# ---- MP4 / WebM -------------------------------------------------------------------------------
# bt709 tags + explicit matrix so players show the same colours as the PNGs. aq-mode 3 spends bits on
# the dark gradients (less banding); +faststart for web playback.
COLOR_TAGS=(-colorspace bt709 -color_primaries bt709 -color_trc bt709 -color_range tv)
yuv_vf() { echo "$(scale_for "$1"):out_color_matrix=bt709:out_range=tv,format=yuv420p"; }

make_mp4() {
  info "mp4: libx264 crf $MP4_CRF"
  heavy "${FF[@]}" "${IN[@]}" -vf "$(yuv_vf "$MP4_WIDTH")" \
    -c:v libx264 -preset slow -crf "$MP4_CRF" -profile:v high -pix_fmt yuv420p \
    -x264-params "aq-mode=3:aq-strength=1.0" "${COLOR_TAGS[@]}" \
    -r "$FPS" -movflags +faststart -an "$TMP/streampad.mp4"
  mv -f "$TMP/streampad.mp4" "$OUT/streampad.mp4"
}

make_webm() {
  info "webm: libvpx-vp9 crf $WEBM_CRF (2-pass)"
  local vf common
  vf=$(yuv_vf "$WEBM_WIDTH")
  common=(-c:v libvpx-vp9 -b:v 0 -crf "$WEBM_CRF" -pix_fmt yuv420p -row-mt 1 -tile-columns 1 -g 150
          -deadline good -cpu-used 1 -auto-alt-ref 1 -lag-in-frames 25 "${COLOR_TAGS[@]}" -r "$FPS" -an)
  heavy "${FF[@]}" "${IN[@]}" -vf "$vf" "${common[@]}" -pass 1 -passlogfile "$TMP/vp9" -f null /dev/null
  heavy "${FF[@]}" "${IN[@]}" -vf "$vf" "${common[@]}" -pass 2 -passlogfile "$TMP/vp9" -f webm "$TMP/streampad.webm"
  mv -f "$TMP/streampad.webm" "$OUT/streampad.webm"
}

# ---- poster / contact sheet -------------------------------------------------------------------
make_poster() {
  local idx=${POSTER_FRAME:-327} src
  if [ -z "${POSTER_FRAME:-}" ] && [ "$idx" -ge $((START + N)) ]; then idx=$((START + N / 2)); fi
  src=$(printf "%s/%0${DIGITS}d.png" "$FRAMES" "$idx")
  [ -f "$src" ] || die "poster frame missing: $src"
  cp -f "$src" "$TMP/poster.png"
  mv -f "$TMP/poster.png" "$OUT/poster.png"
  info "poster: frame $idx"
}

make_contact() {
  # one thumbnail per second, 5 columns
  local secs=$(( (N + FPS - 1) / FPS )) cols=5 rows
  rows=$(( (secs + cols - 1) / cols ))
  heavy "${FF[@]}" "${IN[@]}" \
    -vf "fps=1,scale=384:-2:flags=lanczos,tile=${cols}x${rows}:padding=6:margin=6:color=0x0a0b0e" \
    -frames:v 1 "$TMP/contact.png"
  mv -f "$TMP/contact.png" "$OUT/contact.png"
}

# ---- run --------------------------------------------------------------------------------------
skip gif     || make_gif
skip mp4     || make_mp4
skip webm    || make_webm
skip poster  || make_poster
skip contact || make_contact

# ---- sanity -----------------------------------------------------------------------------------
if [ -n "$(find "$SRC_DIR" -maxdepth 1 -name '*.png' -newer "$TMP/.start" -print -quit)" ]; then
  info "NOTE: frames in $SRC_DIR were re-rendered during the encode; the outputs reflect the snapshot taken at the start - re-run for the newest frames"
fi
# ---- report -----------------------------------------------------------------------------------
echo
echo "== outputs in $OUT"
for f in streampad.gif streampad.mp4 streampad.webm poster.png contact.png; do
  if [ -f "$OUT/$f" ]; then printf '  %-16s %8s MB\n' "$f" "$(mb "$OUT/$f")"; fi
done
if [ -f "$OUT/streampad.gif" ]; then
  IFS=, read -r gw gh gn <<<"$(ffprobe -v error -count_frames -select_streams v:0 \
      -show_entries stream=width,height,nb_read_frames -of csv=p=0 "$OUT/streampad.gif")"
  gd=$(ffprobe -v error -show_entries format=duration -of csv=p=0 "$OUT/streampad.gif")
  # NETSCAPE2.0 application extension = the loop block (gifski/ffmpeg write loop count 0 = forever)
  if LC_ALL=C grep -aq 'NETSCAPE2\.0' "$OUT/streampad.gif"; then loop="loops forever"; else loop="NO loop block (plays once!)"; fi
  echo "  gif: ${gw}x${gh}, ${gn} frames (gifski merges identical frames), ${gd} s, ${loop}"
  if [ "$(awk -v d="$gd" -v e="$SECS" 'BEGIN{x=d-e; if(x<0)x=-x; print (x<0.1)}')" != 1 ]; then
    echo "  WARNING: gif duration ${gd} s differs from the source ${SECS} s" >&2
  fi
fi
