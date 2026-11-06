#include "storylet_engine.h"
#include <fstream>
#include <iostream>
#include <iterator>
#include <string>

static std::string copy_and_free(char *value) {
  if (!value) return {};
  std::string result(value);
  dn_string_free(value);
  return result;
}

int main(int argc, char **argv) {
  const char *path = argc > 1 ? argv[1] : STORYLET_CONTENT_PATH;
  std::ifstream file(path, std::ios::binary);
  if (!file) { std::cerr << "Could not open run content.\n"; return 1; }
  const std::string content((std::istreambuf_iterator<char>(file)), {});
  char *error = nullptr;
  DnRunHandle *run = dn_run_start(content.c_str(), R"({"kind":"beginning"})", 7, &error);
  if (!run) { std::cerr << copy_and_free(error) << '\n'; return 1; }
  const auto before = copy_and_free(dn_run_view(run, 0));
  // This fixture's opening interaction is node 1, first choice.
  const auto after = copy_and_free(dn_run_act(run, 0, R"({"kind":"choose","nodeGuid":1,"outputPin":0})"));
  const bool ok = before.find("\"currentStep\":0") != std::string::npos &&
                  after.find("\"currentStep\":1") != std::string::npos && dn_run_rewind(run, 0) == 0;
  dn_run_free(run);
  if (!ok) { std::cerr << "Run failed: " << after << '\n'; return 1; }
  std::cout << "RunKernel start, act, view, and rewind passed.\n";
  return 0;
}
