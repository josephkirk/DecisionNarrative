#include "storylet_engine.h"

#include <fstream>
#include <iostream>
#include <iterator>
#include <string>

static std::string copy_and_free(DnBuffer buffer) {
  if (buffer.data == nullptr || buffer.len == 0) {
    dn_buffer_free(buffer);
    return {};
  }
  std::string result(reinterpret_cast<const char *>(buffer.data), buffer.len);
  dn_buffer_free(buffer);
  return result;
}

static bool call_succeeded(const std::string &json) {
  return json.find("\"ok\":true") != std::string::npos;
}

int main() {
  std::ifstream project_file(STORYLET_PROJECT_PATH, std::ios::binary);
  if (!project_file) {
    std::cerr << "Could not open the generic sample project.\n";
    return 1;
  }
  const std::string project((std::istreambuf_iterator<char>(project_file)), {});
  const std::string scenario = "opening";

  DnPlaytestHandle *engine = dn_playtest_create();
  if (engine == nullptr) {
    std::cerr << "Could not create a storylet engine handle.\n";
    return 1;
  }

  const std::string started = copy_and_free(dn_playtest_start(
      engine, reinterpret_cast<const uint8_t *>(project.data()), project.size(),
      reinterpret_cast<const uint8_t *>(scenario.data()), scenario.size()));
  if (!call_succeeded(started)) {
    std::cerr << started << '\n';
    dn_playtest_destroy(engine);
    return 1;
  }
  std::cout << "Started generic scenario: " << started << '\n';

  const std::string evaluation = copy_and_free(dn_playtest_evaluate(engine, 0.15));
  if (!call_succeeded(evaluation) || evaluation.find("\"selected\":\"read-receipt\"") == std::string::npos) {
    std::cerr << "Director did not select the expected eligible storylet: " << evaluation << '\n';
    dn_playtest_destroy(engine);
    return 1;
  }
  std::cout << "Director selected: " << evaluation << '\n';

  const std::string choice = R"({"storyletId":"read-receipt","choiceId":"read","idempotencyKey":"native:read-receipt","expectedRevision":0})";
  const std::string applied = copy_and_free(dn_playtest_execute_choice(
      engine, reinterpret_cast<const uint8_t *>(choice.data()), choice.size()));
  if (!call_succeeded(applied) || applied.find("\"clue.receipt\":true") == std::string::npos) {
    std::cerr << "The storylet choice did not apply its authored effect: " << applied << '\n';
    dn_playtest_destroy(engine);
    return 1;
  }
  std::cout << "Choice applied: " << applied << '\n';

  const std::string snapshot = copy_and_free(dn_playtest_snapshot(engine));
  std::cout << "Final session: " << snapshot << '\n';
  dn_playtest_destroy(engine);
  return 0;
}
