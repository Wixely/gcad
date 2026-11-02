#include <filesystem>
// Preserve authored declarations from the source GLB, not reconstructed map bytes.
#include <rapidjson/document.h>
#include <rapidjson/writer.h>
#include <rapidjson/stringbuffer.h>
#include <rapidjson/reader.h>
#include <rapidjson/memorystream.h>
#include <cctype>
#include <algorithm>

// Audit-only ceilings, not accepted production asset budgets. Validate structure
// iteratively before constructing a DOM or recursively walking source metadata.
struct ProvenanceJsonBounds : rapidjson::BaseReaderHandler<rapidjson::UTF8<>,ProvenanceJsonBounds> {
    unsigned depth=0;size_t values=0;bool exceeded=false;
    bool value(){if(++values>1000000){exceeded=true;return false;}return true;}
    bool enter(){if(++depth>64){exceeded=true;return false;}return value();}
    bool StartObject(){return enter();} bool EndObject(rapidjson::SizeType){--depth;return true;}
    bool StartArray(){return enter();} bool EndArray(rapidjson::SizeType){--depth;return true;}
    bool Key(const char*,rapidjson::SizeType,bool){return value();}
    bool String(const char*,rapidjson::SizeType,bool){return value();}
    bool Null(){return value();} bool Bool(bool){return value();}
    bool Int(int){return value();} bool Uint(unsigned){return value();}
    bool Int64(int64_t){return value();} bool Uint64(uint64_t){return value();}
    bool Double(double){return value();}
};

static rapidjson::Document readProvenanceDocument(const char* path) {
    std::ifstream file(std::filesystem::u8path(path),std::ios::binary|std::ios::ate);
    require(bool(file),"cannot read provenance GLB");
    const auto size=file.tellg();file.seekg(0);
    require(size>=0 && size<=std::streamoff(64*1024*1024),"provenance GLB exceeds audit file limit");
    unsigned char header[20]{};file.read(reinterpret_cast<char*>(header),20);
    auto word=[&](unsigned at){return uint32_t(header[at])|(uint32_t(header[at+1])<<8)|(uint32_t(header[at+2])<<16)|(uint32_t(header[at+3])<<24);};
    require(bool(file) && word(0)==0x46546c67 && word(4)==2 && size==std::streamoff(word(8)),"invalid provenance GLB header");
    const auto length=word(12);
    require(word(16)==0x4e4f534a && length%4==0 && length<=16*1024*1024 && uint64_t(length)+20<=uint64_t(size),"invalid provenance JSON chunk");
    std::string json(length,' ');file.read(json.data(),length);require(bool(file),"truncated provenance JSON");
    rapidjson::Reader reader;rapidjson::MemoryStream stream(json.data(),json.size());ProvenanceJsonBounds bounds;
    bool valid=bool(reader.Parse<rapidjson::kParseValidateEncodingFlag|rapidjson::kParseIterativeFlag>(stream,bounds));
    require(!bounds.exceeded,"provenance JSON exceeds audit structural limit");
    require(valid,"invalid provenance JSON");
    rapidjson::Document doc;doc.Parse<rapidjson::kParseValidateEncodingFlag|rapidjson::kParseIterativeFlag>(json.data(),json.size());
    require(!doc.HasParseError() && doc.IsObject(),"invalid provenance JSON");
    return doc;
}

static std::string serializeProvenance(const rapidjson::Value& records) {
    require(records.IsArray() && records.Size()<=4096,"invalid provenance record array");
    const std::set<std::string> slots={"base_colour","roughness","metallic","normal","occlusion","emissive"};
    for(const auto& record:records.GetArray()) {
        require(record.IsObject(),"invalid provenance record");
        std::set<std::string> fields;
        for(auto it=record.MemberBegin();it!=record.MemberEnd();++it)
            require(fields.insert(std::string(it->name.GetString(),it->name.GetStringLength())).second,"duplicate provenance field");
        auto string=[&](const char* key) {
            require(record.HasMember(key) && record[key].IsString(),"missing provenance string field");
            auto value=std::string(record[key].GetString(),record[key].GetStringLength());
            require(!value.empty() && value.find('\0')==std::string::npos,"empty or NUL provenance string");return value;
        };
        require(slots.count(string("slot"))==1,"unknown provenance slot");
        auto relative=string("project_path");
        require(relative.find_first_of("\\:")==std::string::npos,"nonportable provenance path");
        size_t start=0;
        for(;;) {
            auto end=relative.find('/',start);auto component=relative.substr(start,end==std::string::npos?end:end-start);
            require(!component.empty() && component!="." && component!="..","nonportable provenance path");
            if(end==std::string::npos)break;start=end+1;
        }
        auto hash=string("sha256");
        require(hash.size()==64 && std::all_of(hash.begin(),hash.end(),[](unsigned char c){return std::isxdigit(c)!=0;}),"invalid provenance source hash");
        for(auto key:{"license","origin"}) {auto value=string(key);require(std::any_of(value.begin(),value.end(),[](unsigned char c){return !std::isspace(c);}),"blank provenance declaration");}
    }
    rapidjson::StringBuffer buffer;rapidjson::Writer<rapidjson::StringBuffer> writer(buffer);
    require(records.Accept(writer) && buffer.GetSize()>0 && buffer.GetSize()<=1024*1024,"provenance serialization exceeds limit");
    return std::string(buffer.GetString(),buffer.GetSize());
}

static std::string sourceMaterialProvenance(const rapidjson::Value& material) {
    require(material.IsObject(),"source material must be an object");
    if(material.HasMember("extras") && material["extras"].IsObject() &&
       material["extras"].HasMember("gcad_texture_source_inputs"))
        return serializeProvenance(material["extras"]["gcad_texture_source_inputs"]);
    bool textured=material.HasMember("normalTexture") || material.HasMember("occlusionTexture") || material.HasMember("emissiveTexture");
    if(material.HasMember("pbrMetallicRoughness")) {
        const auto& pbr=material["pbrMetallicRoughness"];
        require(pbr.IsObject(),"invalid source PBR material");
        textured=textured || pbr.HasMember("baseColorTexture") || pbr.HasMember("metallicRoughnessTexture");
    }
    require(!textured,"source texture provenance missing");
    // Canonical untextured materials have no source-image declarations.
    return "[]";
}

static std::string readMaterialProvenance(const char* path) {
    auto doc=readProvenanceDocument(path);
    require(doc.HasMember("meshes") && doc["meshes"].IsArray() && doc["meshes"].Size()==1,"provenance probe needs one source mesh");
    const auto& mesh=doc["meshes"][0];
    require(mesh.IsObject() && mesh.HasMember("primitives") && mesh["primitives"].IsArray() && mesh["primitives"].Size()==1,"provenance probe needs one source primitive");
    const auto& primitive=mesh["primitives"][0];
    require(primitive.IsObject() && primitive.HasMember("material") && primitive["material"].IsUint(),"source material reference missing");
    auto index=primitive["material"].GetUint();
    require(doc.HasMember("materials") && doc["materials"].IsArray() && index<doc["materials"].Size(),"source material index invalid");
    const auto& material=doc["materials"][index];
    return sourceMaterialProvenance(material);
}

static void verifyMaterialProvenance(const ufbx_material* material,const std::string& expected) {
    require(ufbx_find_int(&material->props,"gcad|TextureSourceInputsVersion",0)==1,"FBX provenance schema missing");
    auto* prop=ufbx_find_prop(&material->props,"gcad|TextureSourceInputsJson");
    require(prop && std::string(prop->value_str.data,prop->value_str.length)==expected,"FBX provenance missing or changed");
}
