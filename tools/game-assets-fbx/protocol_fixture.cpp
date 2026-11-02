// Test-only executable: behavior is selected by the copied executable filename.
#include <chrono>
#include <filesystem>
#include <iostream>
#include <string>
#include <thread>
#include <fstream>
#include "helper_lease.h"
int main(int argc, char** argv) {
    if(argc==3 && std::string(argv[1])=="--hold-stage-lease") {
        StageLease lease(std::filesystem::u8path(argv[2]));
        std::ofstream(std::filesystem::u8path(argv[2])/"ready")<<"ready";
        std::this_thread::sleep_for(std::chrono::seconds(60));
        return 0;
    }
    if (argc != 2 || std::string(argv[1]) != "--handshake") return 3;
    const auto mode = std::filesystem::path(argv[0]).stem().string();
    if (mode == "timeout" || mode == "cancel") {
        std::this_thread::sleep_for(std::chrono::seconds(60));
    } else if (mode == "stdout") {
        std::cout << std::string(100000, 'x');
    } else if (mode == "stderr") {
        std::cerr << std::string(100000, 'x');
    } else if (mode == "failure") {
        return 7;
    } else {
        std::cout << "{not valid JSON}\n";
    }
    return 0;
}
