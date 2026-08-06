#pragma once

#include "rust/cxx.h"

namespace media_converter {

struct MediaProbeResult;
struct MediaConvertResult;

MediaProbeResult media_probe(rust::Str path);
MediaConvertResult media_convert(rust::Str input, rust::Str output, int preset);

}  // namespace media_converter
