#!/bin/sh
# Re-encodes the recorded take for the landing page's scroll story. Nothing is cut or retimed:
# the only differences from media/demo/public/footage/remote/take.mp4 are the size and a keyframe
# at every beat boundary in src/story.ts, so the page can jump between beats at once.
# Needs ffmpeg. Run from site/ after a retake, and commit the two files it writes.
set -eu
take="../media/demo/public/footage/remote/take.mp4"
keys="0,5.2,5.6,9.2,9.4,15.9,16.3,25.9,26.0,29.8,37.4,46.6"
ffmpeg -v error -y -i "$take" -an -vf "scale=1920:-2:flags=lanczos" -c:v libx264 -preset slow -crf 27 \
  -pix_fmt yuv420p -g 60 -force_key_frames "$keys" -movflags +faststart public/story/take.mp4
ffmpeg -v error -y -ss 5.2 -i "$take" -frames:v 1 -vf "scale=1920:-2:flags=lanczos" -c:v libwebp -quality 86 public/story/poster.webp
ls -l public/story
