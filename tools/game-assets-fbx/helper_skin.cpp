// Read complete FBX influences independently of the consumer's weight cutoff.
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
std::string inspectSkin(const char* input) {
    auto path=std::filesystem::u8path(input);
    need(path.is_absolute(),"skin inspection requires absolute FBX path");
    auto length=std::filesystem::file_size(path);
    need(length>0 && length<=256*1024*1024,"FBX inspection input exceeds bound");
    std::vector<char> bytes(static_cast<size_t>(length));std::ifstream stream(path,std::ios::binary);
    need(bool(stream.read(bytes.data(),static_cast<std::streamsize>(length))) && stream.peek()==std::ifstream::traits_type::eof(),"FBX read failed or changed");
    ufbx_load_opts options{};options.ignore_animation=true;options.load_external_files=false;
    options.temp_allocator.memory_limit=256*1024*1024;options.result_allocator.memory_limit=256*1024*1024;
    ufbx_error error{};
    std::unique_ptr<ufbx_scene,decltype(&ufbx_free_scene)> scene(ufbx_load_memory(bytes.data(),bytes.size(),&options,&error),&ufbx_free_scene);
    need(bool(scene),"FBX skin read failed");
    double unit=scene->settings.unit_meters;need(std::isfinite(unit)&&unit>0,"invalid FBX unit");
    rapidjson::StringBuffer buffer;rapidjson::Writer<rapidjson::StringBuffer> out(buffer);
    out.StartObject();out.Key("protocol");out.Int(1);out.Key("meshes");out.StartArray();
    size_t total=0;
    for(auto* node:scene->nodes) {
        auto* mesh=node->mesh;if(!mesh || !mesh->skin_deformers.count)continue;
        need(mesh->skin_deformers.count==1,"one linear skin per mesh required");
        auto* skin=mesh->skin_deformers.data[0];
        need(skin->skinning_method==UFBX_SKINNING_METHOD_LINEAR,"linear skin required");
        need(skin->vertices.count==mesh->vertices.count,"incomplete skin vertices");
        total+=mesh->vertices.count;need(total<=1000000,"skin vertex count exceeds bound");
        auto name=str(node->name);need(!name.empty()&&name.size()<=4096,"invalid mesh node name");
        out.StartObject();out.Key("name");out.String(name.c_str());
        out.Key("bones");out.StartArray();std::set<std::string> names;
        for(auto* cluster:skin->clusters) {
            need(cluster->bone_node,"missing skin bone");auto bone=str(cluster->bone_node->name);
            need(!bone.empty()&&bone.size()<=4096&&names.insert(bone).second,"skin bone names must be unique");out.String(bone.c_str());
        }
        out.EndArray();out.Key("vertices");out.StartArray();
        for(size_t v=0;v<mesh->vertices.count;v++) {
            auto p=ufbx_transform_position(&node->geometry_to_world,mesh->vertices.data[v]);
            auto range=skin->vertices.data[v];need(range.num_weights>0&&range.num_weights<=4,"one to four influences required");
            out.StartObject();out.Key("position");out.StartArray();
            for(double x:{p.x*unit,p.y*unit,p.z*unit}){need(std::isfinite(x),"nonfinite position");out.Double(x);}out.EndArray();
            double sum=0;std::set<unsigned> used;
            out.Key("joints");out.StartArray();
            for(unsigned i=0;i<range.num_weights;i++) {
                need(size_t(range.weight_begin)+i<skin->weights.count,"invalid influence range");auto w=skin->weights.data[range.weight_begin+i];
                need(w.cluster_index<skin->clusters.count&&used.insert(w.cluster_index).second&&std::isfinite(w.weight)&&w.weight>0,"invalid influence");
                sum+=w.weight;out.Uint(w.cluster_index);
            }out.EndArray();need(std::abs(sum-1)<1e-4,"skin weights must sum to one");
            out.Key("weights");out.StartArray();
            for(unsigned i=0;i<range.num_weights;i++)out.Double(skin->weights.data[range.weight_begin+i].weight/sum);
            out.EndArray();out.EndObject();need(buffer.GetSize()<=64*1024*1024,"skin contract exceeds bound");
        }
        out.EndArray();out.EndObject();
    }
    out.EndArray();out.EndObject();return {buffer.GetString(),buffer.GetSize()};
}
