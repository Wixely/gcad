// Dimension inspection only; this does not inflate pixel data or prove complete
// decoder acceptance. Use the already-pinned Assimp contribution, without I/O.
#define STB_IMAGE_IMPLEMENTATION
#define STBI_ONLY_PNG
#define STBI_ONLY_JPEG
#define STBI_NO_STDIO
#define STBI_NO_HDR
#define STBI_NO_LINEAR
#include <stb_image.h>

static void verifySourceImages(const rapidjson::Document& doc,const char* path) {
    if(!doc.HasMember("images"))return;
    require(doc["images"].IsArray() && doc["images"].Size()<=1024,"source image count exceeds audit limit");
    std::ifstream file(std::filesystem::u8path(path),std::ios::binary|std::ios::ate);auto end=file.tellg();
    require(bool(file) && end>=28 && end<=std::streamoff(64*1024*1024),"source image file size invalid");
    file.seekg(12);unsigned char jsonWord[4]{};file.read(reinterpret_cast<char*>(jsonWord),4);
    uint64_t bin=28+uint64_t(uint32_t(jsonWord[0])|(uint32_t(jsonWord[1])<<8)|(uint32_t(jsonWord[2])<<16)|(uint32_t(jsonWord[3])<<24));
    uint64_t decoded=0,encoded=0;
    for(const auto& image:doc["images"].GetArray()) {
        require(image.IsObject() && !image.HasMember("uri"),"external source image unsupported by audit");
        require(image.HasMember("bufferView") && image["bufferView"].IsUint() && doc.HasMember("bufferViews") &&
            doc["bufferViews"].IsArray() && image["bufferView"].GetUint()<doc["bufferViews"].Size(),"source image view reference invalid");
        require(image.HasMember("mimeType") && image["mimeType"].IsString(),"source image MIME missing");
        std::string mime(image["mimeType"].GetString(),image["mimeType"].GetStringLength());
        require(mime=="image/png" || mime=="image/jpeg","source image MIME unsupported by audit");
        const auto& view=doc["bufferViews"][image["bufferView"].GetUint()];
        require(!view.HasMember("byteStride"),"source image view must not be strided");
        uint64_t start=bin+sourceUnsigned(view,"byteOffset",0),length=sourceUnsigned(view,"byteLength",0);
        require(length>0 && length<=16*1024*1024,"source encoded image exceeds audit limit");
        require(length<=64*1024*1024-encoded,"source encoded image reads exceed audit budget");encoded+=length;
        require(start<=uint64_t(end) && length<=uint64_t(end)-start,"source image outside file");
        std::vector<unsigned char> bytes(size_t(length),0);file.seekg(std::streamoff(start));file.read(reinterpret_cast<char*>(bytes.data()),std::streamsize(length));require(bool(file),"source image read failed");
        static const unsigned char png[]={137,80,78,71,13,10,26,10};
        bool signature=mime=="image/png"?(bytes.size()>=8 && std::equal(png,png+8,bytes.begin())):(bytes.size()>=3 && bytes[0]==255 && bytes[1]==216 && bytes[2]==255);
        require(signature,"source image MIME signature differs");
        int width=0,height=0,channels=0;
        require(stbi_info_from_memory(bytes.data(),int(bytes.size()),&width,&height,&channels)!=0,"source image header invalid");
        require(width>0 && height>0 && width<=8192 && height<=8192,"source image dimensions exceed audit limit");
        uint64_t charge=uint64_t(width)*uint64_t(height)*8; // Conservative RGBA16 estimate.
        require(charge<=128*1024*1024-decoded,"source images exceed audit decoded budget");decoded+=charge;
    }
    std::cout<<"Embedded image headers: "<<doc["images"].Size()<<", estimated decoded bytes "<<decoded<<" (pixels not decoded)\n";
}
