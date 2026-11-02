// Staging-only conversion. The Rust owner decides whether to publish the package.
#include <assimp/Exporter.hpp>
#include <assimp/Importer.hpp>
#include <assimp/metadata.h>
#include <assimp/material.h>
#include <assimp/scene.h>
#include <ufbx.h>
#include <cmath>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <memory>
#include <set>
#include <stdexcept>
#include <string>
#include <cstring>
static void require(bool condition,const char* message) {
    if(!condition)throw std::runtime_error(message);
}
using Scene=std::unique_ptr<ufbx_scene,decltype(&ufbx_free_scene)>;
#include "material_extension_probe.h"
#include "material_provenance_probe.h"
#include "scene_provenance_probe.h"
#include "helper_lease.h"

std::string convertStaged(const char* input,const char* output,bool leased) {
    const auto source=std::filesystem::u8path(input),stage=std::filesystem::u8path(output);
    require(source.is_absolute() && stage.is_absolute(),"conversion paths must be absolute");
    std::unique_ptr<StageLease> sourceLease,outputLease;
    if(leased) {
        sourceLease=std::make_unique<StageLease>(source.parent_path());
        outputLease=std::make_unique<StageLease>(stage);
    }
    require(std::filesystem::is_regular_file(source),"conversion input is not a file");
    require(std::filesystem::is_directory(stage) && !std::filesystem::is_symlink(stage) &&
        std::filesystem::is_empty(stage),"conversion stage must be an existing empty directory");
    // Existing verifier diagnostics go to stderr; stdout is only the response.
    struct Diagnostics {
        std::streambuf* saved=std::cout.rdbuf(std::cerr.rdbuf());
        ~Diagnostics(){std::cout.rdbuf(saved);}
    } diagnostics;
    auto package=convertPreparedScene(input);
    rapidjson::StringBuffer json;
    rapidjson::Writer<rapidjson::StringBuffer> writer(json);
    writer.StartObject();writer.Key("protocol");writer.Uint(1);
    writer.Key("artifacts");writer.StartArray();
    auto write=[&](const std::string& relative,const void* bytes,size_t size) {
        auto path=stage/std::filesystem::u8path(relative);
        path.make_preferred(); // Extended Windows paths do not normalize '/'.
        std::ofstream file(path,std::ios::binary|std::ios::out);
        require(bool(file),"cannot create staged artifact");
        file.write(static_cast<const char*>(bytes),std::streamsize(size));
        file.close();require(bool(file),"cannot finish staged artifact");
        std::ifstream check(path,std::ios::binary|std::ios::ate);
        require(bool(check) && check.tellg()==std::streamoff(size),"staged artifact length mismatch");
        check.seekg(0);std::vector<char> actual(size);check.read(actual.data(),std::streamsize(size));
        require(bool(check) && !std::memcmp(actual.data(),bytes,size),"staged artifact bytes mismatch");
        writer.StartObject();writer.Key("path");writer.String(relative.c_str());
        writer.Key("bytes");writer.Uint64(size);writer.EndObject();
    };
    write("model.fbx",package.fbx.data(),package.fbx.size());
    if(!package.textures.files.empty())require(std::filesystem::create_directory(stage/"textures"),"cannot create staged texture directory");
    for(const auto& texture:package.textures.files)write(texture.relative_path,texture.png.data(),texture.png.size());
    writer.EndArray();writer.EndObject();return std::string(json.GetString(),json.GetSize());
}
