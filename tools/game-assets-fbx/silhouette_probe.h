// Independent scanline coverage check of the exported flex-plate LOD fixture.
#include <array>
using Point2=std::array<double,2>;
using Point3d=std::array<double,3>;
using Face3=std::array<unsigned,3>;

static std::vector<bool> scanCoverage(const std::vector<Point2> &points,const std::vector<Face3> &faces,unsigned resolution) {
    std::vector<bool> mask(resolution*resolution,false);
    for(const auto &face:faces) {
        Point2 p[3]={points[face[0]],points[face[1]],points[face[2]]};
        double area=(p[1][0]-p[0][0])*(p[2][1]-p[0][1])-(p[1][1]-p[0][1])*(p[2][0]-p[0][0]);
        if(area==0) continue;
        int first=std::max(0,int(std::ceil(std::min({p[0][1],p[1][1],p[2][1]})-.5)));
        int last=std::min(int(resolution)-1,int(std::floor(std::max({p[0][1],p[1][1],p[2][1]})-.5)));
        for(int row=first;row<=last;++row) {
            double y=row+.5,low=INFINITY,high=-INFINITY;
            for(unsigned e=0;e<3;++e) {
                const auto &a=p[e],&b=p[(e+1)%3];
                if(y<std::min(a[1],b[1]) || y>std::max(a[1],b[1])) continue;
                if(a[1]==b[1]) {if(y==a[1]) {low=std::min(low,std::min(a[0],b[0]));high=std::max(high,std::max(a[0],b[0]));}}
                else {double x=a[0]+(y-a[1])*(b[0]-a[0])/(b[1]-a[1]);low=std::min(low,x);high=std::max(high,x);}
            }
            if(!std::isfinite(low) || !std::isfinite(high)) continue;
            int left=std::max(0,int(std::ceil(low-.5))),right=std::min(int(resolution)-1,int(std::floor(high-.5)));
            for(int col=left;col<=right;++col) mask[row*resolution+col]=true;
        }
    }
    return mask;
}

static std::vector<Face3> importedFaces(const aiMesh *mesh) {
    std::vector<Face3> result;
    for(unsigned f=0;f<mesh->mNumFaces;++f) {
        require(mesh->mFaces[f].mNumIndices==3,"silhouette input is not triangulated");
        auto *i=mesh->mFaces[f].mIndices;result.push_back({i[0],i[1],i[2]});
    }
    return result;
}

static std::vector<Point3d> flexPositions(const aiScene *scene,double time) {
    auto *plate=scene->mRootNode->FindNode("plate");
    auto *mesh=scene->mMeshes[plate->mMeshes[0]];
    auto *tip=scene->mRootNode->FindNode("tip");
    auto *base=scene->mRootNode->FindNode("base");
    aiVector3D scale,translation;aiQuaternion rotation;
    tip->mTransformation.Decompose(scale,rotation,translation);
    if(time>=0) {
        auto *channel=scene->mAnimations[0]->mChannels[0];
        aiQuaternion::Interpolate(rotation,channel->mRotationKeys[0].mValue,channel->mRotationKeys[1].mValue,float(time));rotation.Normalize();
    }
    auto baseWorld=worldMatrix(base),tipWorld=baseWorld*aiMatrix4x4(scale,rotation,translation);
    std::vector<aiVector3D> points(mesh->mNumVertices,aiVector3D(0,0,0));
    for(unsigned b=0;b<mesh->mNumBones;++b) {
        auto *bone=mesh->mBones[b];auto matrix=(std::string(bone->mName.C_Str())=="tip"?tipWorld:baseWorld)*bone->mOffsetMatrix;
        for(unsigned w=0;w<bone->mNumWeights;++w) {auto weight=bone->mWeights[w];points[weight.mVertexId]+=(matrix*mesh->mVertices[weight.mVertexId])*weight.mWeight;}
    }
    std::vector<Point3d> result;
    for(auto p:points) result.push_back({double(p.x)*1000,-double(p.z)*1000,double(p.y)*1000});
    return result;
}

struct CoverageResult {unsigned source,target,different,united;};
static CoverageResult compareCoverage(const std::vector<Point3d> &a,const std::vector<Face3> &fa,const std::vector<Point3d> &b,const std::vector<Face3> &fb,unsigned axis) {
    unsigned axes[2],k=0;for(unsigned i=0;i<3;++i) if(i!=axis) axes[k++]=i;
    Point2 low={INFINITY,INFINITY},high={-INFINITY,-INFINITY};
    for(const auto *points:{&a,&b}) for(auto p:*points) for(unsigned j=0;j<2;++j) {low[j]=std::min(low[j],p[axes[j]]);high[j]=std::max(high[j],p[axes[j]]);}
    double span=std::max(high[0]-low[0],high[1]-low[1]),scale=span>0?span/252:1;
    auto project=[&](const std::vector<Point3d> &points) {
        std::vector<Point2> result;for(auto p:points) result.push_back({(p[axes[0]]-(low[0]+high[0])/2)/scale+128,(p[axes[1]]-(low[1]+high[1])/2)/scale+128});return result;
    };
    auto ma=scanCoverage(project(a),fa,256),mb=scanCoverage(project(b),fb,256);
    CoverageResult result={};
    for(unsigned i=0;i<ma.size();++i) {result.source+=ma[i];result.target+=mb[i];result.different+=ma[i]!=mb[i];result.united+=ma[i]||mb[i];}
    return result;
}

static void verifySilhouettes(const char *reference,const char *lod) {
    // Known masks ensure agreement is not merely two empty/broken rasters.
    std::vector<Point2> square={{2,2},{62,2},{2,62},{62,62}};
    std::vector<Face3> diagonal={{0,1,2},{1,3,2}},other={{0,1,3},{0,3,2}};
    auto full=scanCoverage(square,diagonal,64),alternate=scanCoverage(square,other,64);
    require(full==alternate && std::count(full.begin(),full.end(),true)==3600,"scanline calibration failed");
    auto half=scanCoverage(square,{{0,1,2}},64);auto halfCount=std::count(half.begin(),half.end(),true);
    require(halfCount>=1770 && halfCount<=1830,"scanline missing-coverage calibration failed");
    Assimp::Importer sourceImporter,lodImporter;
    const auto *source=sourceImporter.ReadFile(reference,0),*target=lodImporter.ReadFile(lod,0);
    verifyPlateSkin(source,true);verifyPlateSkin(target,true);
    auto *sm=source->mMeshes[source->mRootNode->FindNode("plate")->mMeshes[0]],*tm=target->mMeshes[target->mRootNode->FindNode("plate")->mMeshes[0]];
    require(sm->mNumFaces==20416 && tm->mNumFaces==20000,"silhouette fixture triangle counts changed");
    auto sf=importedFaces(sm),tf=importedFaces(tm);
    unsigned views=0,totalDifference=0,minimumCoverage=256*256;
    for(int sample=-1;sample<=16;++sample) {
        double time=sample<0?-1:sample/16.0;
        auto a=flexPositions(source,time),b=flexPositions(target,time);
        for(unsigned axis=0;axis<3;++axis) {
            auto r=compareCoverage(a,sf,b,tf,axis);require(r.source>0 && r.target>0,"unresolved silhouette view");
            require(r.different==0,"exported LOD silhouette differs from the reference");
            minimumCoverage=std::min(minimumCoverage,std::min(r.source,r.target));totalDifference+=r.different;++views;
        }
        if(sample==0) {
            for(auto &p:b) p[0]+=1;
            require(compareCoverage(a,sf,b,tf,2).different>0,"silhouette comparison missed a displaced mesh");
        }
    }
    std::cout << "Independent scanline silhouettes passed: " << views << " shared 256x256 views, " << totalDifference << " differing pixels; minimum occupied pixels " << minimumCoverage << "; displacement negative control passed\n";
}
