// Read the versioned contract from FBX without relying on an engine's importer.
#include <ufbx.h>
#include <rapidjson/stringbuffer.h>
#include <rapidjson/writer.h>
#include <cmath>
#include <filesystem>
#include <fstream>
#include <memory>
#include <set>
#include <stdexcept>
#include <string>
#include <vector>

static void need(bool ok,const char* message) { if(!ok)throw std::runtime_error(message); }
static std::string str(ufbx_string value) { return {value.data,value.length}; }

std::string inspectMaterials(const char* input) {
    auto path=std::filesystem::u8path(input);
    need(path.is_absolute(),"material inspection requires an absolute FBX path");
    auto length=std::filesystem::file_size(path);
    need(length>0 && length<=256*1024*1024,"FBX inspection input exceeds bound");
    std::vector<char> bytes(static_cast<size_t>(length));
    std::ifstream stream(path,std::ios::binary);
    need(bool(stream.read(bytes.data(),static_cast<std::streamsize>(length))),"cannot read FBX inspection input");
    need(stream.peek()==std::ifstream::traits_type::eof(),"FBX changed during read");
    ufbx_load_opts options{};
    options.ignore_geometry=true;options.ignore_animation=true;
    options.load_external_files=false;
    options.temp_allocator.memory_limit=256*1024*1024;
    options.result_allocator.memory_limit=256*1024*1024;
    ufbx_error error{};
    std::unique_ptr<ufbx_scene,decltype(&ufbx_free_scene)> scene(
        ufbx_load_memory(bytes.data(),bytes.size(),&options,&error),&ufbx_free_scene);
    need(bool(scene),"FBX material read failed");
    need(scene->materials.count>0 && scene->materials.count<=1024,"material count outside bounds");
    std::set<const ufbx_material*> attached;
    for(auto* node:scene->nodes)for(auto* material:node->materials)attached.insert(material);
    need(!attached.empty(),"no attached FBX materials");
    rapidjson::StringBuffer buffer;
    rapidjson::Writer<rapidjson::StringBuffer> out(buffer);
    out.StartObject();out.Key("protocol");out.Int(1);out.Key("materialSchema");out.Int(3);
    out.Key("materials");out.StartArray();
    std::set<std::string> names;
    for(auto* material:scene->materials) {
        if(!attached.count(material))continue; // Assimp also emits an unused default material.
        auto name=str(material->name);
        need(!name.empty() && name.size()<=4096 && names.insert(name).second,"material names must be unique and bounded");
        auto prop=[&](const std::string& name) {
            auto* value=ufbx_find_prop(&material->props,name.c_str());
            if(!value)throw std::runtime_error("required gcad material property absent: "+name+" in "+str(material->name));
            return value;
        };
        auto number=[&](const std::string& name) {
            double value=prop(name)->value_real;need(std::isfinite(value),"nonfinite material property");return value;
        };
        need(prop("gcad|PbrVersion")->value_int==3,"unsupported gcad material schema");
        out.StartObject();out.Key("name");out.String(name.c_str(),static_cast<rapidjson::SizeType>(name.size()));
        for(const char* key:{"Roughness","Metallic","NormalScale","OcclusionStrength","AlphaCutoff","BaseAlpha"}) {
            out.Key(key);out.Double(number(std::string("gcad|")+key));
        }
        auto mode=str(prop("gcad|AlphaMode")->value_str);
        need(mode=="OPAQUE" || mode=="MASK" || mode=="BLEND","invalid alpha mode");
        out.Key("AlphaMode");out.String(mode.c_str());
        auto sides=prop("gcad|DoubleSided")->value_int;
        need(sides==0 || sides==1,"invalid double-sided flag");
        out.Key("DoubleSided");out.Bool(sides!=0);
        for(const char* key:{"BaseColour","Emission"}) {
            auto value=prop(std::string("gcad|")+key)->value_vec3;
            out.Key(key);out.StartArray();
            for(double x:{value.x,value.y,value.z}) {need(std::isfinite(x),"nonfinite material colour");out.Double(x);}
            out.EndArray();
        }
        out.Key("maps");out.StartArray();
        for(const char* slot:{"BaseColour","Roughness","Metallic","Normal","Occlusion","Emission"}) {
            const std::string prefix=std::string("gcad|")+slot;
            const ufbx_texture* texture=nullptr;
            for(const auto& connection:material->textures)if(str(connection.material_prop)==prefix) {
                need(texture==nullptr,"duplicate custom material texture slot");texture=connection.texture;
            }
            if(!texture)continue;
            auto filename=str(texture->relative_filename);
            need(!texture->content.size,"inspection requires external PNGs");
            const std::string start="textures/image_",end=".png";
            need(filename.size()>start.size()+end.size() && filename.compare(0,start.size(),start)==0 &&
                filename.compare(filename.size()-end.size(),end.size(),end)==0,"noncanonical material texture path");
            auto digits=filename.substr(start.size(),filename.size()-start.size()-end.size());
            need(digits.size()<=4 && digits.find_first_not_of("0123456789")==std::string::npos &&
                (digits.size()==1 || digits[0]!='0') && std::stoul(digits)<1024,"invalid texture image index");
            out.StartObject();out.Key("slot");out.String(slot);out.Key("path");out.String(filename.c_str());
            for(const char* key:{"WrapU","WrapV","MagFilter","MinFilter","UvSet"}) {
                out.Key(key);out.Int64(prop(prefix+"|"+key)->value_int);
            }
            if(prefix=="gcad|Roughness" || prefix=="gcad|Metallic" || prefix=="gcad|Occlusion") {
                out.Key("channel");out.Int64(prop(prefix+"Channel")->value_int);
            }
            out.EndObject();
        }
        out.EndArray();out.EndObject();
        need(buffer.GetSize()<=4*1024*1024,"material contract output exceeds bound");
    }
    out.EndArray();out.EndObject();
    return {buffer.GetString(),buffer.GetSize()};
}
