#pragma once

#include "rust/cxx.h"

namespace yse_ui {

struct MediaProbeResult;
struct MediaConvertResult;

MediaProbeResult media_probe(rust::Str path);
MediaConvertResult media_convert(rust::Str input, rust::Str output, int preset);

}  // namespace yse_ui
