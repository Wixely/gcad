// Independent PNG decoding and chart-space sampling of the CAD-box atlas.
#define STB_IMAGE_STATIC
#define STB_IMAGE_IMPLEMENTATION
#define STBI_ONLY_PNG
#include <stb_image.h>
#include <array>
#include <memory>

struct AtlasImage {
    int width=0,height=0;
    std::vector<unsigned char> pixels;
};
static AtlasImage atlasImage(const aiScene* scene,const aiMaterial* material,aiTextureType type) {
    aiString path;
    require(material->GetTexture(type,0,&path)==AI_SUCCESS,"atlas texture binding missing");
    auto* texture=scene->GetEmbeddedTexture(path.C_Str());
    require(texture && texture->mHeight==0,"atlas expected embedded compressed PNG");
    AtlasImage image; int channels=0;
    auto* pixels=stbi_load_from_memory(reinterpret_cast<const unsigned char*>(texture->pcData),
        texture->mWidth,&image.width,&image.height,&channels,4);
    require(pixels!=nullptr,"independent PNG decoding failed");
    image.pixels.assign(pixels,pixels+image.width*image.height*4);
    stbi_image_free(pixels);
    return image;
}
static int atlasAddress(double uv,int size,aiTextureMapMode mode) {
    if(mode==aiTextureMapMode_Clamp) uv=std::clamp(uv,0.0,1.0);
    else if(mode==aiTextureMapMode_Mirror) {
        uv=std::fmod(uv,2.0); if(uv<0)uv+=2; if(uv>1)uv=2-uv;
    } else {uv-=std::floor(uv);}
    return std::clamp(int(std::floor(uv*size)),0,size-1);
}
static void verifyAtlas(const char* originalPath,const char* atlasPath) {
    Assimp::Importer originalImporter,atlasImporter;
    auto* original=originalImporter.ReadFile(originalPath,0);
    auto* atlas=atlasImporter.ReadFile(atlasPath,0);
    require(original && atlas && original->mNumMeshes==1 && atlas->mNumMeshes==1,"atlas scenes changed");
    auto* source=original->mMeshes[0]; auto* packed=atlas->mMeshes[0];
    require(source->mNumVertices==24 && packed->mNumVertices==24 && source->mNumFaces==12 && packed->mNumFaces==12,"CAD box topology changed");
    require(source->HasTextureCoords(0) && packed->HasTextureCoords(0) && packed->HasTangentsAndBitangents(),"atlas attributes missing");
    for(unsigned i=0;i<24;++i) {
        require((source->mVertices[i]-packed->mVertices[i]).Length()<1e-7,"atlas geometry changed");
        require((source->mNormals[i]-packed->mNormals[i]).Length()<1e-7,"atlas normals changed");
        require((source->mTangents[i]-packed->mTangents[i]).Length()<1e-6,"atlas tangent basis changed");
    }
    for(unsigned f=0;f<12;++f) for(unsigned k=0;k<3;++k)
        require(source->mFaces[f].mIndices[k]==packed->mFaces[f].mIndices[k],"atlas indices changed");
    std::vector<std::vector<unsigned>> groups;
    for(unsigned i=0;i<24;++i) {
        size_t g=0;
        while(g<groups.size() && (source->mNormals[i]-source->mNormals[groups[g][0]]).Length()>1e-6) ++g;
        if(g==groups.size())groups.push_back({});
        groups[g].push_back(i);
    }
    require(groups.size()==6,"CAD box chart count changed");
    auto* sm=original->mMaterials[source->mMaterialIndex];
    auto* am=atlas->mMaterials[packed->mMaterialIndex];
    unsigned samples=0,gutterSamples=0,skipped=0;
    for(auto type:{aiTextureType_BASE_COLOR,aiTextureType_METALNESS,aiTextureType_NORMALS,aiTextureType_LIGHTMAP,aiTextureType_EMISSIVE}) {
        auto oldImage=atlasImage(original,sm,type),newImage=atlasImage(atlas,am,type);
        require(oldImage.width==2 && oldImage.height==2 && newImage.width==256 && newImage.height==128,"atlas image dimensions changed");
        aiTextureMapMode oldModes[3]={aiTextureMapMode_Wrap,aiTextureMapMode_Wrap,aiTextureMapMode_Wrap};
        aiTextureMapMode newModes[3]={aiTextureMapMode_Wrap,aiTextureMapMode_Wrap,aiTextureMapMode_Wrap}; aiString path;
        sm->GetTexture(type,0,&path,nullptr,nullptr,nullptr,nullptr,oldModes);
        am->GetTexture(type,0,&path,nullptr,nullptr,nullptr,nullptr,newModes);
        require(newModes[0]==aiTextureMapMode_Clamp && newModes[1]==aiTextureMapMode_Clamp,"atlas sampler is not clamped");
        int magnify=0,minify=0;
        require(am->Get(AI_MATKEY_GLTF_MAPPINGFILTER_MAG(type,0),magnify)==AI_SUCCESS && magnify==9728,"atlas magnification changed");
        require(am->Get(AI_MATKEY_GLTF_MAPPINGFILTER_MIN(type,0),minify)==AI_SUCCESS && minify==9728,"atlas minification must be nearest without mipmaps");
        if(type==aiTextureType_BASE_COLOR) {
            require(oldImage.pixels[0]==220 && oldImage.pixels[1]==40 && oldImage.pixels[2]==20 && oldImage.pixels[3]==80,"source PNG calibration failed");
        }
        for(const auto& group:groups) {
            require(group.size()==4,"CAD box chart does not have four vertices");
            double low[2]={1e30,1e30},high[2]={-1e30,-1e30},oldLow[2]={1e30,1e30},oldHigh[2]={-1e30,-1e30};
            for(auto i:group) for(unsigned k=0;k<2;++k) {
                double u=k?1-packed->mTextureCoords[0][i].y:packed->mTextureCoords[0][i].x;
                double v=k?1-source->mTextureCoords[0][i].y:source->mTextureCoords[0][i].x;
                low[k]=std::min(low[k],u); high[k]=std::max(high[k],u);
                oldLow[k]=std::min(oldLow[k],v); oldHigh[k]=std::max(oldHigh[k],v);
            }
            int left=int(std::floor(low[0]*256)),right=int(std::floor(high[0]*256));
            int top=int(std::floor(low[1]*128)),bottom=int(std::floor(high[1]*128));
            require(left>=2 && top>=2 && right+2<256 && bottom+2<128,"atlas padding exceeds image");
            for(int y=top-2;y<=bottom+2;++y) for(int x=left-2;x<=right+2;++x) {
                double u=oldLow[0]+((x+.5)/256-low[0])*(oldHigh[0]-oldLow[0])/(high[0]-low[0]);
                double v=oldLow[1]+((y+.5)/128-low[1])*(oldHigh[1]-oldLow[1])/(high[1]-low[1]);
                // Float32 imported UVs can land on either side of nearest-sample ties.
                if(std::abs(u*2-std::round(u*2))<1e-5 || std::abs(v*2-std::round(v*2))<1e-5) {++skipped;continue;}
                int sx=atlasAddress(u,2,oldModes[0]),sy=atlasAddress(v,2,oldModes[1]);
                for(unsigned c=0;c<4;++c) require(newImage.pixels[4*(y*256+x)+c]==oldImage.pixels[4*(sy*2+sx)+c],"atlas chart or padding pixel differs from source sampling");
                ++samples; if(x<left || x>right || y<top || y>bottom)++gutterSamples;
            }
        }
    }
    require(samples>10000 && gutterSamples>1000,"insufficient atlas pixel coverage");
    std::cout<<"Atlas import passed: six CAD charts, five packed textures, "<<samples<<" exact RGBA samples including "<<gutterSamples<<" padding samples; "<<skipped<<" nearest-boundary samples excluded\n";
}
