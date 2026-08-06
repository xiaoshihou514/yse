#include "yse-media-converter/cpp/media.h"

#include <algorithm>
#include <cstdio>
#include <sstream>
#include <string>
#include <vector>

extern "C" {
#include <libavcodec/avcodec.h>
#include <libavfilter/avfilter.h>
#include <libavfilter/buffersink.h>
#include <libavfilter/buffersrc.h>
#include <libavformat/avformat.h>
#include <libavutil/avutil.h>
#include <libavutil/channel_layout.h>
#include <libavutil/error.h>
#include <libavutil/opt.h>
#include <libavutil/pixdesc.h>
#include <libavutil/samplefmt.h>
}

#include "yse-media-converter/src/main.cxx.h"

namespace media_converter {
namespace {

std::string from_str(rust::Str value)
{
  return std::string(value.data(), value.size());
}

std::string error_text(int code)
{
  char text[AV_ERROR_MAX_STRING_SIZE] = {};
  av_strerror(code, text, sizeof(text));
  return text;
}

struct StreamState {
  int input_index = -1;
  int output_index = -1;
  AVCodecContext* decoder = nullptr;
  AVCodecContext* encoder = nullptr;
  AVFilterGraph* graph = nullptr;
  AVFilterContext* source = nullptr;
  AVFilterContext* sink = nullptr;
};

struct Conversion {
  AVFormatContext* input = nullptr;
  AVFormatContext* output = nullptr;
  std::vector<StreamState> streams;
  AVPacket* packet = nullptr;
  AVPacket* encoded = nullptr;
  AVFrame* decoded = nullptr;
  AVFrame* filtered = nullptr;
  std::string output_path;
  bool header_written = false;
  bool succeeded = false;

  ~Conversion()
  {
    av_packet_free(&packet);
    av_packet_free(&encoded);
    av_frame_free(&decoded);
    av_frame_free(&filtered);
    for (auto& stream : streams) {
      avfilter_graph_free(&stream.graph);
      avcodec_free_context(&stream.decoder);
      avcodec_free_context(&stream.encoder);
    }
    avformat_close_input(&input);
    if (output != nullptr) {
      if (!(output->oformat->flags & AVFMT_NOFILE) && output->pb != nullptr) {
        avio_closep(&output->pb);
      }
      avformat_free_context(output);
    }
    if (!succeeded && !output_path.empty()) {
      std::remove(output_path.c_str());
    }
  }
};

const AVCodec* named_encoder(const char* name, AVCodecID fallback)
{
  const AVCodec* codec = avcodec_find_encoder_by_name(name);
  return codec != nullptr ? codec : avcodec_find_encoder(fallback);
}

const AVCodec* encoder_for(int preset, AVMediaType type)
{
  if (type == AVMEDIA_TYPE_VIDEO) {
    if (preset == 0) return named_encoder("libx264", AV_CODEC_ID_H264);
    if (preset == 1) return named_encoder("libvpx-vp9", AV_CODEC_ID_VP9);
    return nullptr;
  }
  if (type == AVMEDIA_TYPE_AUDIO) {
    if (preset == 0) return avcodec_find_encoder(AV_CODEC_ID_AAC);
    if (preset == 1) return named_encoder("libopus", AV_CODEC_ID_OPUS);
    if (preset == 2) return named_encoder("libmp3lame", AV_CODEC_ID_MP3);
    if (preset == 3) return avcodec_find_encoder(AV_CODEC_ID_FLAC);
  }
  return nullptr;
}

int open_decoder(Conversion& job, int index, StreamState& state)
{
  AVStream* stream = job.input->streams[index];
  const AVCodec* codec = avcodec_find_decoder(stream->codecpar->codec_id);
  if (codec == nullptr) return AVERROR_DECODER_NOT_FOUND;
  state.decoder = avcodec_alloc_context3(codec);
  if (state.decoder == nullptr) return AVERROR(ENOMEM);
  int result = avcodec_parameters_to_context(state.decoder, stream->codecpar);
  if (result < 0) return result;
  state.decoder->pkt_timebase = stream->time_base;
  return avcodec_open2(state.decoder, codec, nullptr);
}

AVPixelFormat choose_pixel_format(const AVCodec* codec)
{
  const AVPixelFormat* formats = nullptr;
  if (avcodec_get_supported_config(nullptr, codec, AV_CODEC_CONFIG_PIX_FORMAT, 0,
                                   reinterpret_cast<const void**>(&formats), nullptr) >= 0 &&
      formats != nullptr) {
    for (const AVPixelFormat* format = formats; *format != AV_PIX_FMT_NONE; ++format) {
      if (*format == AV_PIX_FMT_YUV420P) return *format;
    }
    return formats[0];
  }
  return AV_PIX_FMT_YUV420P;
}

AVSampleFormat choose_sample_format(const AVCodec* codec)
{
  const AVSampleFormat* formats = nullptr;
  if (avcodec_get_supported_config(nullptr, codec, AV_CODEC_CONFIG_SAMPLE_FORMAT, 0,
                                   reinterpret_cast<const void**>(&formats), nullptr) >= 0 &&
      formats != nullptr) {
    return formats[0];
  }
  return AV_SAMPLE_FMT_FLTP;
}

int choose_sample_rate(const AVCodec* codec, int wanted)
{
  const int* rates = nullptr;
  int count = 0;
  if (avcodec_get_supported_config(nullptr, codec, AV_CODEC_CONFIG_SAMPLE_RATE, 0,
                                   reinterpret_cast<const void**>(&rates), &count) < 0 ||
      rates == nullptr || count == 0) {
    return wanted > 0 ? wanted : 48000;
  }
  int best = rates[0];
  for (int i = 1; i < count; ++i) {
    if (std::abs(rates[i] - wanted) < std::abs(best - wanted)) best = rates[i];
  }
  return best;
}

int configure_video_filter(Conversion& job, StreamState& state)
{
  state.graph = avfilter_graph_alloc();
  if (state.graph == nullptr) return AVERROR(ENOMEM);
  AVStream* input_stream = job.input->streams[state.input_index];
  AVRational aspect = state.decoder->sample_aspect_ratio.num != 0
      ? state.decoder->sample_aspect_ratio : input_stream->sample_aspect_ratio;
  char args[512];
  std::snprintf(args, sizeof(args),
                "video_size=%dx%d:pix_fmt=%d:time_base=%d/%d:pixel_aspect=%d/%d",
                state.decoder->width, state.decoder->height, state.decoder->pix_fmt,
                input_stream->time_base.num, input_stream->time_base.den,
                aspect.num == 0 ? 1 : aspect.num, aspect.den == 0 ? 1 : aspect.den);
  int result = avfilter_graph_create_filter(&state.source, avfilter_get_by_name("buffer"),
                                             "source", args, nullptr, state.graph);
  if (result < 0) return result;
  result = avfilter_graph_create_filter(&state.sink, avfilter_get_by_name("buffersink"),
                                         "sink", nullptr, nullptr, state.graph);
  if (result < 0) return result;
  AVFilterContext* format = nullptr;
  const char* pixel_name = av_get_pix_fmt_name(state.encoder->pix_fmt);
  std::string format_args = std::string("pix_fmts=") + (pixel_name ? pixel_name : "yuv420p");
  result = avfilter_graph_create_filter(&format, avfilter_get_by_name("format"), "format",
                                         format_args.c_str(), nullptr, state.graph);
  if (result < 0) return result;
  if ((result = avfilter_link(state.source, 0, format, 0)) < 0) return result;
  if ((result = avfilter_link(format, 0, state.sink, 0)) < 0) return result;
  return avfilter_graph_config(state.graph, nullptr);
}

int configure_audio_filter(StreamState& state)
{
  state.graph = avfilter_graph_alloc();
  if (state.graph == nullptr) return AVERROR(ENOMEM);
  char layout[128];
  av_channel_layout_describe(&state.decoder->ch_layout, layout, sizeof(layout));
  const char* sample_name = av_get_sample_fmt_name(state.decoder->sample_fmt);
  char args[512];
  std::snprintf(args, sizeof(args),
                "time_base=1/%d:sample_rate=%d:sample_fmt=%s:channel_layout=%s",
                state.decoder->sample_rate, state.decoder->sample_rate,
                sample_name ? sample_name : "fltp", layout);
  int result = avfilter_graph_create_filter(&state.source, avfilter_get_by_name("abuffer"),
                                             "source", args, nullptr, state.graph);
  if (result < 0) return result;
  result = avfilter_graph_create_filter(&state.sink, avfilter_get_by_name("abuffersink"),
                                         "sink", nullptr, nullptr, state.graph);
  if (result < 0) return result;
  AVFilterContext* format = nullptr;
  char output_layout[128];
  av_channel_layout_describe(&state.encoder->ch_layout, output_layout, sizeof(output_layout));
  const char* output_sample = av_get_sample_fmt_name(state.encoder->sample_fmt);
  char format_args[512];
  std::snprintf(format_args, sizeof(format_args),
                "sample_fmts=%s:sample_rates=%d:channel_layouts=%s",
                output_sample, state.encoder->sample_rate, output_layout);
  result = avfilter_graph_create_filter(&format, avfilter_get_by_name("aformat"), "format",
                                         format_args, nullptr, state.graph);
  if (result < 0) return result;
  if ((result = avfilter_link(state.source, 0, format, 0)) < 0) return result;
  if ((result = avfilter_link(format, 0, state.sink, 0)) < 0) return result;
  result = avfilter_graph_config(state.graph, nullptr);
  if (result >= 0 && state.encoder->frame_size > 0) {
    av_buffersink_set_frame_size(state.sink, state.encoder->frame_size);
  }
  return result;
}

int add_stream(Conversion& job, int input_index, int preset)
{
  StreamState state;
  auto fail = [&state](int code) {
    avfilter_graph_free(&state.graph);
    avcodec_free_context(&state.decoder);
    avcodec_free_context(&state.encoder);
    return code;
  };
  state.input_index = input_index;
  int result = open_decoder(job, input_index, state);
  if (result < 0) return fail(result);
  AVMediaType type = state.decoder->codec_type;
  const AVCodec* codec = encoder_for(preset, type);
  if (codec == nullptr) return fail(AVERROR_ENCODER_NOT_FOUND);
  state.encoder = avcodec_alloc_context3(codec);
  if (state.encoder == nullptr) return fail(AVERROR(ENOMEM));
  AVStream* input_stream = job.input->streams[input_index];
  AVStream* output_stream = avformat_new_stream(job.output, nullptr);
  if (output_stream == nullptr) return fail(AVERROR(ENOMEM));
  state.output_index = output_stream->index;

  if (type == AVMEDIA_TYPE_VIDEO) {
    state.encoder->width = state.decoder->width;
    state.encoder->height = state.decoder->height;
    state.encoder->sample_aspect_ratio = state.decoder->sample_aspect_ratio;
    state.encoder->pix_fmt = choose_pixel_format(codec);
    AVRational rate = av_guess_frame_rate(job.input, input_stream, nullptr);
    if (rate.num == 0) rate = AVRational{25, 1};
    state.encoder->framerate = rate;
    state.encoder->time_base = av_inv_q(rate);
    state.encoder->bit_rate = preset == 1 ? 0 : 2'500'000;
    if (preset == 0 && state.encoder->priv_data != nullptr) {
      av_opt_set(state.encoder->priv_data, "preset", "medium", 0);
      av_opt_set(state.encoder->priv_data, "crf", "23", 0);
    } else if (preset == 1 && state.encoder->priv_data != nullptr) {
      av_opt_set(state.encoder->priv_data, "crf", "32", 0);
      av_opt_set(state.encoder->priv_data, "b", "0", 0);
    }
  } else {
    state.encoder->sample_fmt = choose_sample_format(codec);
    state.encoder->sample_rate = choose_sample_rate(codec, state.decoder->sample_rate);
    if (state.decoder->ch_layout.nb_channels > 0) {
      av_channel_layout_copy(&state.encoder->ch_layout, &state.decoder->ch_layout);
    } else {
      av_channel_layout_default(&state.encoder->ch_layout, 2);
    }
    state.encoder->time_base = AVRational{1, state.encoder->sample_rate};
    state.encoder->bit_rate = preset == 3 ? 0 : (preset == 2 ? 192000 : 160000);
  }
  if (job.output->oformat->flags & AVFMT_GLOBALHEADER) {
    state.encoder->flags |= AV_CODEC_FLAG_GLOBAL_HEADER;
  }
  result = avcodec_open2(state.encoder, codec, nullptr);
  if (result < 0) return fail(result);
  result = avcodec_parameters_from_context(output_stream->codecpar, state.encoder);
  if (result < 0) return fail(result);
  output_stream->time_base = state.encoder->time_base;
  result = type == AVMEDIA_TYPE_VIDEO
      ? configure_video_filter(job, state) : configure_audio_filter(state);
  if (result < 0) return fail(result);
  job.streams.push_back(state);
  return 0;
}

StreamState* find_state(Conversion& job, int input_index)
{
  for (auto& stream : job.streams) {
    if (stream.input_index == input_index) return &stream;
  }
  return nullptr;
}

int drain_encoder(Conversion& job, StreamState& state)
{
  for (;;) {
    int result = avcodec_receive_packet(state.encoder, job.encoded);
    if (result == AVERROR(EAGAIN) || result == AVERROR_EOF) return 0;
    if (result < 0) return result;
    AVStream* output_stream = job.output->streams[state.output_index];
    av_packet_rescale_ts(job.encoded, state.encoder->time_base, output_stream->time_base);
    job.encoded->stream_index = state.output_index;
    result = av_interleaved_write_frame(job.output, job.encoded);
    av_packet_unref(job.encoded);
    if (result < 0) return result;
  }
}

int encode_filtered(Conversion& job, StreamState& state)
{
  for (;;) {
    int result = av_buffersink_get_frame(state.sink, job.filtered);
    if (result == AVERROR(EAGAIN) || result == AVERROR_EOF) return 0;
    if (result < 0) return result;
    job.filtered->pts = av_rescale_q(job.filtered->pts,
                                     av_buffersink_get_time_base(state.sink),
                                     state.encoder->time_base);
    if (state.encoder->codec_type == AVMEDIA_TYPE_VIDEO) {
      job.filtered->pict_type = AV_PICTURE_TYPE_NONE;
    }
    result = avcodec_send_frame(state.encoder, job.filtered);
    av_frame_unref(job.filtered);
    if (result < 0) return result;
    if ((result = drain_encoder(job, state)) < 0) return result;
  }
}

int drain_decoder(Conversion& job, StreamState& state)
{
  for (;;) {
    int result = avcodec_receive_frame(state.decoder, job.decoded);
    if (result == AVERROR(EAGAIN) || result == AVERROR_EOF) return 0;
    if (result < 0) return result;
    job.decoded->pts = job.decoded->best_effort_timestamp;
    result = av_buffersrc_add_frame_flags(state.source, job.decoded,
                                           AV_BUFFERSRC_FLAG_KEEP_REF);
    av_frame_unref(job.decoded);
    if (result < 0) return result;
    if ((result = encode_filtered(job, state)) < 0) return result;
  }
}

MediaConvertResult fail_result(const std::string& operation, int code)
{
  return MediaConvertResult{false, rust::String(operation + ": " + error_text(code))};
}

}  // namespace

MediaProbeResult media_probe(rust::Str path_value)
{
  std::string path = from_str(path_value);
  AVFormatContext* context = nullptr;
  int result = avformat_open_input(&context, path.c_str(), nullptr, nullptr);
  if (result < 0) {
    return MediaProbeResult{false, rust::String("Could not open file: " + error_text(result)),
                            0, false, false};
  }
  result = avformat_find_stream_info(context, nullptr);
  if (result < 0) {
    avformat_close_input(&context);
    return MediaProbeResult{false, rust::String("Could not inspect file: " + error_text(result)),
                            0, false, false};
  }
  bool video = false;
  bool audio = false;
  std::ostringstream summary;
  if (context->iformat != nullptr && context->iformat->long_name != nullptr) {
    summary << context->iformat->long_name;
  } else {
    summary << "Media file";
  }
  for (unsigned i = 0; i < context->nb_streams; ++i) {
    AVCodecParameters* parameters = context->streams[i]->codecpar;
    const AVCodecDescriptor* descriptor = avcodec_descriptor_get(parameters->codec_id);
    const char* codec = descriptor != nullptr ? descriptor->name : "unknown";
    if (parameters->codec_type == AVMEDIA_TYPE_VIDEO && !video) {
      video = true;
      summary << " · " << parameters->width << "×" << parameters->height << " " << codec;
    } else if (parameters->codec_type == AVMEDIA_TYPE_AUDIO && !audio) {
      audio = true;
      summary << " · " << parameters->sample_rate << " Hz " << codec;
    }
  }
  int64_t duration = context->duration == AV_NOPTS_VALUE
      ? 0 : context->duration / (AV_TIME_BASE / 1000);
  if (duration > 0) summary << " · " << duration / 1000.0 << " s";
  avformat_close_input(&context);
  return MediaProbeResult{true, rust::String(summary.str()), duration, video, audio};
}

MediaConvertResult media_convert(rust::Str input_value, rust::Str output_value, int preset)
{
  if (preset < 0 || preset > 3) {
    return MediaConvertResult{false, rust::String("Unknown output profile")};
  }
  Conversion job;
  std::string input_path = from_str(input_value);
  job.output_path = from_str(output_value);
  if (input_path.empty() || job.output_path.empty()) {
    return MediaConvertResult{false, rust::String("Input and output paths are required")};
  }
  if (input_path == job.output_path) {
    return MediaConvertResult{false, rust::String("Output must be different from input")};
  }
  if (FILE* existing = std::fopen(job.output_path.c_str(), "rb")) {
    std::fclose(existing);
    return MediaConvertResult{false, rust::String("Output already exists")};
  }
  int result = avformat_open_input(&job.input, input_path.c_str(), nullptr, nullptr);
  if (result < 0) return fail_result("Could not open input", result);
  result = avformat_find_stream_info(job.input, nullptr);
  if (result < 0) return fail_result("Could not inspect input", result);
  result = avformat_alloc_output_context2(&job.output, nullptr, nullptr, job.output_path.c_str());
  if (result < 0 || job.output == nullptr) {
    return fail_result("Could not choose output container", result < 0 ? result : AVERROR(EINVAL));
  }

  int video_index = preset <= 1
      ? av_find_best_stream(job.input, AVMEDIA_TYPE_VIDEO, -1, -1, nullptr, 0) : -1;
  int audio_index = av_find_best_stream(job.input, AVMEDIA_TYPE_AUDIO, -1, -1, nullptr, 0);
  if (video_index < 0 && audio_index < 0) {
    return MediaConvertResult{false, rust::String("No usable audio or video stream")};
  }
  if (video_index >= 0) {
    result = add_stream(job, video_index, preset);
    if (result < 0) return fail_result("Could not configure video", result);
  }
  if (audio_index >= 0) {
    result = add_stream(job, audio_index, preset);
    if (result < 0) return fail_result("Could not configure audio", result);
  }
  if (!(job.output->oformat->flags & AVFMT_NOFILE)) {
    result = avio_open(&job.output->pb, job.output_path.c_str(), AVIO_FLAG_WRITE);
    if (result < 0) return fail_result("Could not create output", result);
  }
  result = avformat_write_header(job.output, nullptr);
  if (result < 0) return fail_result("Could not write output header", result);
  job.header_written = true;
  job.packet = av_packet_alloc();
  job.encoded = av_packet_alloc();
  job.decoded = av_frame_alloc();
  job.filtered = av_frame_alloc();
  if (!job.packet || !job.encoded || !job.decoded || !job.filtered) {
    return fail_result("Could not allocate conversion buffers", AVERROR(ENOMEM));
  }

  while ((result = av_read_frame(job.input, job.packet)) >= 0) {
    StreamState* state = find_state(job, job.packet->stream_index);
    if (state != nullptr) {
      result = avcodec_send_packet(state->decoder, job.packet);
      av_packet_unref(job.packet);
      if (result < 0) return fail_result("Could not decode packet", result);
      result = drain_decoder(job, *state);
      if (result < 0) return fail_result("Could not process decoded frame", result);
    } else {
      av_packet_unref(job.packet);
    }
  }
  if (result != AVERROR_EOF) return fail_result("Could not read input", result);

  for (auto& state : job.streams) {
    result = avcodec_send_packet(state.decoder, nullptr);
    if (result < 0) return fail_result("Could not flush decoder", result);
    if ((result = drain_decoder(job, state)) < 0) return fail_result("Could not flush frames", result);
    result = av_buffersrc_add_frame_flags(state.source, nullptr, 0);
    if (result < 0) return fail_result("Could not flush filter", result);
    if ((result = encode_filtered(job, state)) < 0) return fail_result("Could not flush filter", result);
    result = avcodec_send_frame(state.encoder, nullptr);
    if (result < 0) return fail_result("Could not flush encoder", result);
    if ((result = drain_encoder(job, state)) < 0) return fail_result("Could not finish output", result);
  }
  result = av_write_trailer(job.output);
  if (result < 0) return fail_result("Could not write output trailer", result);
  job.succeeded = true;
  return MediaConvertResult{true, rust::String("Conversion complete")};
}

}  // namespace media_converter
