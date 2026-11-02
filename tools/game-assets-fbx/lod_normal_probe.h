// Independent UV-space normal comparison for a coplanar CAD LOD fixture.
struct LodNormalSample {
    bool covered=false;
    aiVector3D position,normal,tangent,bitangent;
};
static std::vector<LodNormalSample> lodNormalRaster(const aiMesh* mesh,int size) {
    require(mesh->HasTextureCoords(0) && mesh->HasNormals() && mesh->HasTangentsAndBitangents(),"LOD normal attributes missing");
    std::vector<LodNormalSample> result(size*size);
    for(unsigned f=0;f<mesh->mNumFaces;++f) {
        const auto& face=mesh->mFaces[f];require(face.mNumIndices==3,"LOD normal face is not triangular");
        double u[3],v[3];
        for(unsigned k=0;k<3;++k){auto uv=mesh->mTextureCoords[0][face.mIndices[k]];u[k]=uv.x*size;v[k]=(1.0-uv.y)*size;}
        double det=(u[1]-u[0])*(v[2]-v[0])-(v[1]-v[0])*(u[2]-u[0]);
        require(std::abs(det)>1e-15,"LOD normal UV triangle collapsed");
        int left=std::max(0,int(std::floor(std::min({u[0],u[1],u[2]}))));
        int right=std::min(size-1,int(std::floor(std::max({u[0],u[1],u[2]}))));
        int top=std::max(0,int(std::floor(std::min({v[0],v[1],v[2]}))));
        int bottom=std::min(size-1,int(std::floor(std::max({v[0],v[1],v[2]}))));
        for(int y=top;y<=bottom;++y)for(int x=left;x<=right;++x) {
            double dx=x+.5-u[0],dy=y+.5-v[0];
            double b=(dx*(v[2]-v[0])-dy*(u[2]-u[0]))/det;
            double c=((u[1]-u[0])*dy-(v[1]-v[0])*dx)/det;
            double a=1-b-c;
            if(std::min({a,b,c})< -1e-6)continue;
            auto& sample=result[y*size+x];if(sample.covered)continue;
            a=std::max(0.,a);b=std::max(0.,b);c=std::max(0.,c);double sum=a+b+c;
            float weights[3]={float(a/sum),float(b/sum),float(c/sum)};
            for(unsigned k=0;k<3;++k) {
                auto i=face.mIndices[k];
                sample.position+=mesh->mVertices[i]*weights[k];sample.normal+=mesh->mNormals[i]*weights[k];
                sample.tangent+=mesh->mTangents[i]*weights[k];sample.bitangent+=mesh->mBitangents[i]*weights[k];
            }
            sample.normal.Normalize();
            sample.tangent-=sample.normal*(sample.normal*sample.tangent);sample.tangent.Normalize();
            auto cross=sample.normal^sample.tangent;
            sample.bitangent=cross*((cross*sample.bitangent)<0?-1.f:1.f);
            sample.covered=true;
        }
    }
    return result;
}
static double lodNormalAngle(aiVector3D a,aiVector3D b) {
    a.Normalize();b.Normalize();return std::acos(std::clamp(double(a*b),-1.,1.))*180/3.141592653589793;
}
static aiVector3D lodProjectedNormal(const aiMesh* high,const LodNormalSample& low) {
    using D=aiVector3t<double>;
    auto d=[](aiVector3D p){return D(p.x,p.y,p.z);};
    D origin=d(low.position),direction=d(low.normal);
    struct Candidate{double distance;D normal;};std::vector<Candidate> hits;
    double nearest=1.;
    for(unsigned f=0;f<high->mNumFaces;++f) {
        auto* ids=high->mFaces[f].mIndices;
        D a=d(high->mVertices[ids[0]]),e=d(high->mVertices[ids[1]])-a,g=d(high->mVertices[ids[2]])-a;
        D plane=e^g;double denominator=plane*direction;
        if(std::abs(denominator)<1e-18)continue;
        double t=(plane*(a-origin))/denominator;
        if(std::abs(t)>.001)continue;
        D point=origin+direction*t-a;
        double ee=e*e,eg=e*g,gg=g*g,pe=point*e,pg=point*g,det=ee*gg-eg*eg;
        if(det<=0)continue;
        double b=(pe*gg-pg*eg)/det,c=(pg*ee-pe*eg)/det,aWeight=1-b-c;
        if(std::min({aWeight,b,c})< -1e-5)continue;
        D normal=d(high->mNormals[ids[0]])*aWeight+d(high->mNormals[ids[1]])*b+d(high->mNormals[ids[2]])*c;
        normal.Normalize();hits.push_back({std::abs(t),normal});nearest=std::min(nearest,std::abs(t));
    }
    require(!hits.empty(),"independent LOD normal projection missed");
    double best=-2.;D chosen;
    // Float32 import and interpolation uncertainty: fixed 0.02 micrometre band.
    for(auto hit:hits)if(hit.distance<=nearest+2e-8 && hit.normal*direction>best) {best=hit.normal*direction;chosen=hit.normal;}
    return aiVector3D(float(chosen.x),float(chosen.y),float(chosen.z));
}
static void verifyLodNormals(const char* sourcePath,const char* lodPath) {
    Assimp::Importer sourceImporter,lodImporter;
    auto* source=sourceImporter.ReadFile(sourcePath,0);auto* lod=lodImporter.ReadFile(lodPath,0);
    require(source && lod && source->mNumMeshes==1 && lod->mNumMeshes==1,"LOD normal scenes changed");
    auto* original=source->mMeshes[0];auto* reduced=lod->mMeshes[0];
    require(original->mNumFaces==5104 && (reduced->mNumFaces==3000 || reduced->mNumFaces==5000),"LOD normal fixture did not reduce to target");
    auto image=atlasImage(lod,lod->mMaterials[reduced->mMaterialIndex],aiTextureType_NORMALS);
    require(image.width==256 && image.height==256,"LOD normal image dimensions changed");
    auto high=lodNormalRaster(original,256),low=lodNormalRaster(reduced,256);
    unsigned samples=0,changed=0,bad=0;double before=0,after=0,maxPosition=0;
    for(size_t i=0;i<low.size();++i) {
        require(low[i].covered==high[i].covered,"LOD normal UV coverage changed");
        if(!low[i].covered)continue;
        ++samples;
        maxPosition=std::max(maxPosition,double((low[i].position-high[i].position).Length()));
        auto* rgba=&image.pixels[i*4];
        auto n=low[i].tangent*(rgba[0]/255.f*2-1)+low[i].bitangent*(rgba[1]/255.f*2-1)+low[i].normal*(rgba[2]/255.f*2-1);
        auto reference=lodProjectedNormal(original,low[i]);
        double oldAngle=lodNormalAngle(low[i].normal,reference),newAngle=lodNormalAngle(n,reference);
        if(newAngle>1. && bad++<5) std::cout<<"bad pixel "<<i%256<<","<<i/256<<" source normal "<<high[i].normal.x<<","<<high[i].normal.y<<","<<high[i].normal.z<<" RGB "<<int(rgba[0])<<","<<int(rgba[1])<<","<<int(rgba[2])<<" error "<<newAngle<<"\n";
        before=std::max(before,oldAngle);after=std::max(after,newAngle);if(oldAngle>1.)++changed;
    }
    std::cout<<"LOD normal comparison: "<<samples<<" covered pixels; "<<changed<<" changed by over one degree before baking; maximum normal error "<<before<<" -> "<<after<<" degrees; maximum position disagreement "<<maxPosition<<" m\n";
    require(samples==1349,"LOD normal sample count changed");
    if(reduced->mNumFaces==5000) require(maxPosition<1e-6,"coplanar UV comparison moved");
    else require(changed>0 && before>1.,"fixture does not exercise normal correction");
    require(after<0.5,"baked LOD normals differ from the original surface");
}
