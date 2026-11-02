// Independent semantic import check of examples/game_asset_glb.rs output.
#include <assimp/Importer.hpp>
#include <assimp/material.h>
#include <assimp/GltfMaterial.h>
#include <assimp/scene.h>
#include <algorithm>
#include <array>
#include <cmath>
#include <iostream>
#include <limits>
#include <stdexcept>
#include <string>
#include <vector>

static void require(bool condition, const char *message) {
    if (!condition) throw std::runtime_error(message);
}

static aiMatrix4x4 worldMatrix(const aiNode *node) {
    aiMatrix4x4 result;
    for (auto *current = node; current; current = current->mParent) result = current->mTransformation * result;
    return result;
}

static void verifyScene(const aiScene *scene) {
    auto *mirror = scene->mRootNode->FindNode("mirror");
    auto *copy = scene->mRootNode->FindNode("copy");
    auto *assembly = scene->mRootNode->FindNode("assembly");
    require(mirror && copy && assembly, "named scene nodes missing");
    require(mirror->mParent == assembly && copy->mParent == assembly, "parent hierarchy changed");
    require(scene->mNumMeshes == 2 && mirror->mNumMeshes == 2 && copy->mNumMeshes == 2, "primitive count changed");
    require(mirror->mMeshes[0] == copy->mMeshes[0] && mirror->mMeshes[1] == copy->mMeshes[1], "mesh instances were not shared");
    require(scene->mMeshes[0]->mMaterialIndex != scene->mMeshes[1]->mMaterialIndex, "material slots collapsed");
    for (unsigned instance = 0; instance < 2; ++instance) {
        auto *node = instance == 0 ? mirror : copy;
        auto world = worldMatrix(node);
        float determinant = world.Determinant();
        require(instance == 0 ? determinant < 0 : determinant > 0, "reflection changed");
        aiMatrix3x3 normalMatrix(world);
        normalMatrix.Inverse().Transpose();
        aiMatrix3x3 tangentMatrix(world);
        float low[3] = {INFINITY, INFINITY, INFINITY}, high[3] = {-INFINITY, -INFINITY, -INFINITY};
        unsigned triangles = 0;
        for (unsigned p = 0; p < node->mNumMeshes; ++p) {
            auto *mesh = scene->mMeshes[node->mMeshes[p]];
            require(mesh->HasNormals() && mesh->HasTangentsAndBitangents(), "scene attributes missing");
            triangles += mesh->mNumFaces;
            for (unsigned f = 0; f < mesh->mNumFaces; ++f) {
                const auto &face = mesh->mFaces[f];
                require(face.mNumIndices == 3, "scene has non-triangle faces");
                aiVector3D points[3];
                for (unsigned k = 0; k < 3; ++k) {
                    auto vertex = face.mIndices[k];
                    points[k] = world * mesh->mVertices[vertex];
                    for (unsigned axis = 0; axis < 3; ++axis) {
                        low[axis] = std::min(low[axis], points[k][axis]);
                        high[axis] = std::max(high[axis], points[k][axis]);
                    }
                    auto normal = normalMatrix * mesh->mNormals[vertex];
                    auto tangent = tangentMatrix * mesh->mTangents[vertex];
                    normal.Normalize(); tangent.Normalize();
                    require(std::abs(normal * tangent) < 1e-5, "nonuniform scale breaks normal/tangent orthogonality");
                }
                auto normal = normalMatrix * mesh->mNormals[face.mIndices[0]];
                auto geometric = (points[1] - points[0]) ^ (points[2] - points[0]);
                require((geometric * normal) * determinant > 0, "reflection winding and inverse-transpose normals disagree");
            }
        }
        require(triangles == 12, "scene triangle count changed");
        const float minimum[2][3] = {{-0.75f, 0, -4}, {-0.25f, 0, -1.5f}};
        const float maximum[2][3] = {{0.75f, 1, -2}, {0.25f, 0.25f, -0.5f}};
        for (unsigned k = 0; k < 3; ++k) {
            require(std::abs(low[k] - minimum[instance][k]) < 1e-5 && std::abs(high[k] - maximum[instance][k]) < 1e-5, "composed world bounds or unit conversion changed");
        }
    }
    auto *material = scene->mMaterials[scene->mMeshes[mirror->mMeshes[1]]->mMaterialIndex];
    aiColor4D colour;
    require(material->Get(AI_MATKEY_BASE_COLOR, colour) == AI_SUCCESS && std::abs(colour.r - 0.2f) < 1e-6 && std::abs(colour.b - 0.8f) < 1e-6, "second primitive material changed");
    for (unsigned p = 0; p < 2; ++p) {
        auto *m = scene->mMaterials[scene->mMeshes[mirror->mMeshes[p]]->mMaterialIndex];
        aiString path;
        require(m->GetTexture(aiTextureType_BASE_COLOR, 0, &path) == AI_SUCCESS && scene->GetEmbeddedTexture(path.C_Str()), "scene material texture index invalid");
    }
    std::cout << "GLB scene import passed: named hierarchy, shared two-primitive mesh, material slots, mirrored/nonuniform transforms, analytic world bounds and normal/tangent orientation\n";
}

// Analytic continuous deformation for the authored plate's explicit y blend.
static aiVector3D plateDeformed(const aiVector3D &p, double time, bool flex, double halfWidth) {
    double x=p.x, y=-p.z, z=p.y;
    double weight=std::clamp((y+halfWidth)/(2*halfWidth),0.0,1.0);
    double angle=time*3.14159265358979323846/2;
    double tx=x*std::cos(angle)-(y-.015)*std::sin(angle);
    double ty=.015+x*std::sin(angle)+(y-.015)*std::cos(angle);
    double tz=z;
    if (flex) {
        tx=x;
        ty=y*std::cos(angle)-(z-.0015)*std::sin(angle);
        tz=.0015+y*std::sin(angle)+(z-.0015)*std::cos(angle);
    }
    return aiVector3D(float(x*(1-weight)+tx*weight),float(z*(1-weight)+tz*weight),float(-(y*(1-weight)+ty*weight)));
}

static void verifyPlateSkin(const aiScene *scene, bool flex=false, double halfWidth=.015, bool eased=false) {
    require(scene && scene->mNumAnimations==1, "expected one authored bend clip");
    auto *plate=scene->mRootNode->FindNode("plate");
    auto *base=scene->mRootNode->FindNode("base");
    auto *tip=scene->mRootNode->FindNode("tip");
    require(plate && base && tip && tip->mParent==base && plate->mNumMeshes==1,"plate skin hierarchy changed");
    auto *mesh=scene->mMeshes[plate->mMeshes[0]];
    require(mesh->mNumVertices>100 && mesh->mNumFaces>100 && mesh->mNumBones==2,"CAD skin geometry missing");
    require(mesh->HasNormals() && mesh->HasTangentsAndBitangents() && mesh->HasTextureCoords(0),"CAD skin attributes missing");
    std::vector<float> weights[2];
    for(auto &w:weights) w.resize(mesh->mNumVertices);
    aiMatrix4x4 offsets[2]; bool seen[2]={};
    for(unsigned b=0;b<mesh->mNumBones;++b) {
        auto *bone=mesh->mBones[b]; std::string name=bone->mName.C_Str();
        require(name=="base" || name=="tip","unknown CAD bone");
        unsigned j=name=="base"?0:1;
        require(!seen[j],"duplicate CAD bone"); seen[j]=true; offsets[j]=bone->mOffsetMatrix;
        for(unsigned k=0;k<bone->mNumWeights;++k) {
            auto w=bone->mWeights[k];
            require(w.mVertexId<mesh->mNumVertices && w.mWeight>0 && weights[j][w.mVertexId]==0,"invalid CAD influence");
            weights[j][w.mVertexId]=w.mWeight;
        }
    }
    for(unsigned i=0;i<mesh->mNumVertices;++i) {
        double w=std::clamp((-double(mesh->mVertices[i].z)+halfWidth)/(2*halfWidth),0.0,1.0);
        require(std::abs(weights[1][i]-w)<1e-6 && std::abs(weights[0][i]-(1-w))<1e-6,"CAD blend weights changed");
    }
    auto *clip=scene->mAnimations[0];
    require(std::string(clip->mName.C_Str())=="bend" && clip->mNumChannels==1 && clip->mTicksPerSecond>0 && std::abs(clip->mDuration/clip->mTicksPerSecond-1)<1e-6,"CAD clip changed");
    auto *channel=clip->mChannels[0];
    require(std::string(channel->mNodeName.C_Str())=="tip" && (eased?(channel->mNumRotationKeys>=2 && channel->mNumRotationKeys<33):channel->mNumRotationKeys==2),"CAD channel changed");
    auto &a=channel->mRotationKeys[0]; auto &b=channel->mRotationKeys[channel->mNumRotationKeys-1];
    require(std::abs(a.mTime)<1e-9 && std::abs(b.mTime/clip->mTicksPerSecond-1)<1e-6,"CAD key times changed");
    double maxError=0,minArea=INFINITY,maxArea=0,maxEdge=0;
    std::vector<double> surfaceErrors;
    std::vector<aiVector3D> posed(mesh->mNumVertices);
    unsigned intervals=eased?128:16;
    for(unsigned sample=0;sample<=intervals;++sample) {
        double time=sample/double(intervals),deformationTime=eased?time*time:time;
        aiVector3D scale,translation; aiQuaternion rotation;
        tip->mTransformation.Decompose(scale,rotation,translation);
        double ticks=time*clip->mTicksPerSecond;
        unsigned lower=0;
        while(lower+1<channel->mNumRotationKeys && channel->mRotationKeys[lower+1].mTime<=ticks) ++lower;
        unsigned upper=std::min(lower+1,channel->mNumRotationKeys-1);
        auto &ka=channel->mRotationKeys[lower]; auto &kb=channel->mRotationKeys[upper];
        float fraction=lower==upper?0.f:float((ticks-ka.mTime)/(kb.mTime-ka.mTime));
        aiQuaternion::Interpolate(rotation,ka.mValue,kb.mValue,fraction); rotation.Normalize();
        auto baseWorld=worldMatrix(base);
        aiMatrix4x4 matrices[2]={baseWorld*offsets[0],baseWorld*aiMatrix4x4(scale,rotation,translation)*offsets[1]};
        for(unsigned i=0;i<mesh->mNumVertices;++i) {
            posed[i]=(matrices[0]*mesh->mVertices[i])*weights[0][i]+(matrices[1]*mesh->mVertices[i])*weights[1][i];
            double error=(posed[i]-plateDeformed(mesh->mVertices[i],deformationTime,flex,halfWidth)).Length();
            require(std::isfinite(error) && error<(eased?6e-5:1e-6),"CAD skin differs from analytic deformation");
            maxError=std::max(maxError,error);
        }
        for(unsigned f=0;f<mesh->mNumFaces;++f) {
            auto &face=mesh->mFaces[f]; require(face.mNumIndices==3,"CAD skin non-triangle");
            auto i=face.mIndices[0],j=face.mIndices[1],k=face.mIndices[2];
            auto p=mesh->mVertices[i],q=mesh->mVertices[j],r=mesh->mVertices[k];
            double restArea=((q-p)^(r-p)).Length(); require(restArea>0,"degenerate CAD rest triangle");
            double area=((posed[j]-posed[i])^(posed[k]-posed[i])).Length()/restArea;
            require(std::isfinite(area),"nonfinite posed triangle area");
            minArea=std::min(minArea,area); maxArea=std::max(maxArea,area);
            for(unsigned e=0;e<3;++e) {
                auto u=face.mIndices[e],v=face.mIndices[(e+1)%3];
                double stretch=(posed[v]-posed[u]).Length()/(mesh->mVertices[v]-mesh->mVertices[u]).Length();
                require(std::isfinite(stretch),"nonfinite posed edge stretch");
                maxEdge=std::max(maxEdge,stretch);
            }
            auto restCenter=(p+q+r)/3.0f;
            auto posedCenter=(posed[i]+posed[j]+posed[k])/3.0f;
            surfaceErrors.push_back((posedCenter-plateDeformed(restCenter,deformationTime,flex,halfWidth)).Length());
        }
    }
    std::sort(surfaceErrors.begin(),surfaceErrors.end());
    std::cout << "Authored CAD skin import passed: " << mesh->mNumVertices << " vertices, " << mesh->mNumFaces
        << " triangles, " << intervals+1 << " poses, " << channel->mNumRotationKeys << " keys; maximum vertex error " << maxError << " m\n"
        << "Deformation diagnostics (not quality acceptance): area ratio [" << minArea << ", " << maxArea
        << "], maximum edge stretch " << maxEdge << "; centroid surface error max " << surfaceErrors.back()*1000
        << " mm, p95 " << surfaceErrors[(surfaceErrors.size()-1)*95/100]*1000 << " mm\n";
}

#include "silhouette_probe.h"
#include "atlas_probe.h"
#include "atlas_minify_probe.h"
#include "normal_probe.h"
#include "lod_normal_probe.h"
#include "root_motion_probe.h"
#include "rigid_motion_probe.h"
#include "attachments_probe.h"
#include "maps_probe.h"
#include "crease_probe.h"
#include "nested_probe.h"
#include "quantize_probe.h"
#include "face_material_probe.h"

int main(int argc, char **argv) {
    try {
        if(argc==4 && std::string(argv[3])=="--atlas-minify") {verifyAtlasMinification(argv[1],argv[2]);return 0;}
        if(argc==3 && std::string(argv[2])=="--nested-assembly") {verifyNested(argv[1]);return 0;}
        if(argc==3 && std::string(argv[2])=="--nested-skin") {verifyNestedSkin(argv[1]);return 0;}
        if(argc==5 && std::string(argv[3])=="--face-material") {verifyFaceMaterial(argv[1],argv[2],argv[4]);return 0;}
        if(argc==5 && std::string(argv[3])=="--face-roughness") {verifyFaceMaterial(argv[1],argv[2],argv[4],true);return 0;}
        if(argc==5 && std::string(argv[3])=="--face-uv") {verifyFaceMaterial(argv[1],argv[2],argv[4],false,true);return 0;}
        if(argc==5 && std::string(argv[3])=="--face-weights") {verifyFaceMaterial(argv[1],argv[2],argv[4],false,false,true);return 0;}
        if(argc==6 && (std::string(argv[3])=="--quantized-weights" || std::string(argv[3])=="--quantized-raw")) {
            verifyQuantizedWeights(argv[1],argv[2],std::stod(argv[4]),std::stod(argv[5]),
                std::string(argv[3])=="--quantized-raw");return 0;
        }
        if(argc==4 && std::string(argv[3])=="--crease-normals") {verifyCreaseNormals(argv[1],argv[2]);return 0;}
        if(argc==3 && std::string(argv[2])=="--maps") {verifyAuthoredMaps(argv[1]);return 0;}
        if(argc==3 && std::string(argv[2])=="--surfaces") {
            Assimp::Importer importer;
            auto *scene=importer.ReadFile(argv[1],0);
            require(scene,"authored surface scene missing");
            auto *masked=scene->mRootNode->FindNode("masked"), *blended=scene->mRootNode->FindNode("blended");
            require(masked && blended && masked->mNumMeshes==1 && blended->mNumMeshes==1,"surface instances missing");
            for(unsigned index=0;index<2;++index) {
                auto *node=index==0?masked:blended;
                auto *material=scene->mMaterials[scene->mMeshes[node->mMeshes[0]]->mMaterialIndex];
                aiColor4D colour; aiString alpha; int twoSided=-1;
                const float expected[2][4]={{.1f,.2f,.3f,.4f},{.6f,.4f,.2f,.5f}};
                require(material->Get(AI_MATKEY_BASE_COLOR,colour)==AI_SUCCESS,"surface colour missing");
                for(unsigned k=0;k<4;++k) require(std::abs(colour[k]-expected[index][k])<1e-6,"linear surface factor changed");
                require(material->Get(AI_MATKEY_GLTF_ALPHAMODE,alpha)==AI_SUCCESS && std::string(alpha.C_Str())==(index==0?"MASK":"BLEND"),"surface alpha mode changed");
                require(material->Get(AI_MATKEY_TWOSIDED,twoSided)==AI_SUCCESS && twoSided==(index==0?1:0),"surface sidedness changed");
                if(index==0) {
                    float metallic=0,roughness=0,cutoff=0; aiColor3D emission;
                    require(material->Get(AI_MATKEY_METALLIC_FACTOR,metallic)==AI_SUCCESS && std::abs(metallic-.25f)<1e-6,"surface metallic changed");
                    require(material->Get(AI_MATKEY_ROUGHNESS_FACTOR,roughness)==AI_SUCCESS && std::abs(roughness-.75f)<1e-6,"surface roughness changed");
                    require(material->Get(AI_MATKEY_GLTF_ALPHACUTOFF,cutoff)==AI_SUCCESS && std::abs(cutoff-.3f)<1e-6,"surface cutoff changed");
                    require(material->Get(AI_MATKEY_COLOR_EMISSIVE,emission)==AI_SUCCESS && std::abs(emission.r-.05f)<1e-6 && std::abs(emission.g-.1f)<1e-6 && std::abs(emission.b-.15f)<1e-6,"surface emission changed");
                }
            }
            std::cout<<"Authored surfaces import passed: linear RGBA, metallic/roughness/emission, MASK/BLEND and sidedness\n";
            return 0;
        }
        if (argc==4 && (std::string(argv[3])=="--twist-sliced" || std::string(argv[3])=="--twist-sliced-fine")) {
            Assimp::Importer originalImporter, slicedImporter;
            auto *original=originalImporter.ReadFile(argv[1],0);
            auto *sliced=slicedImporter.ReadFile(argv[2],0);
            auto slicedSurface=verifyTwistChain(sliced,true,std::string(argv[3])=="--twist-sliced-fine");
            require(original && original->mNumMeshes>0,"slice reference missing");
            auto originalSurface=verifyTwistChain(original);
            std::cout<<"Sliced/reference sampled maximum correspondence ratio "<<slicedSurface.maximum/originalSurface.maximum<<"\n";
            auto measure=[](const aiMesh *mesh) {
                double area=0,volume=0;
                for(unsigned f=0;f<mesh->mNumFaces;++f) {
                    const auto &face=mesh->mFaces[f]; require(face.mNumIndices==3,"non-triangle reference");
                    auto a=mesh->mVertices[face.mIndices[0]], b=mesh->mVertices[face.mIndices[1]], c=mesh->mVertices[face.mIndices[2]];
                    area+=((b-a)^(c-a)).Length()*.5;
                    volume+=double(a*(b^c))/6;
                }
                return std::make_pair(area,volume);
            };
            auto *referencePlate=original->mRootNode->FindNode("plate");
            require(referencePlate && referencePlate->mNumMeshes==1,"slice reference plate missing");
            auto a=measure(original->mMeshes[referencePlate->mMeshes[0]]);
            auto b=measure(sliced->mMeshes[sliced->mRootNode->FindNode("plate")->mMeshes[0]]);
            require(a.first>0 && std::abs(a.second)>0,"invalid rest reference");
            double areaError=std::abs(a.first-b.first)/a.first,volumeError=std::abs(a.second-b.second)/std::abs(a.second);
            require(areaError<1e-5 && volumeError<1e-5,"slice rest area or volume changed");
            std::cout<<"Slice rest relative area error "<<areaError<<", signed volume error "<<volumeError<<"\n";
            return 0;
        }
        if (argc==3 && std::string(argv[2])=="--twist-chain") {
            Assimp::Importer importer;
            verifyTwistChain(importer.ReadFile(argv[1],0));
            return 0;
        }
        if (argc==3 && std::string(argv[2])=="--attachments") {
            Assimp::Importer importer;
            verifyAttachments(importer.ReadFile(argv[1],0));
            return 0;
        }
        if(argc==3 && std::string(argv[2])=="--rigid-motion") {verifyRigidMotion(argv[1]);return 0;}
        if(argc==3 && std::string(argv[2])=="--root-motion") {verifyRootMotion(argv[1]);return 0;}
        if(argc==4 && std::string(argv[3])=="--lod-normals") {verifyLodNormals(argv[1],argv[2]);return 0;}
        if(argc==3 && std::string(argv[2])=="--normal-detail") {verifyNormalDetail(argv[1]);return 0;}
        if(argc==4 && std::string(argv[3])=="--atlas") {verifyAtlas(argv[1],argv[2]);return 0;}
        if(argc==4 && std::string(argv[3])=="--silhouette") {verifySilhouettes(argv[1],argv[2]);return 0;}
        if (argc==3 && (std::string(argv[2])=="--linkage-motion" || std::string(argv[2])=="--linkage-adaptive" || std::string(argv[2])=="--linkage-reduced")) {
            bool adaptive=std::string(argv[2])=="--linkage-adaptive";
            bool reduced=std::string(argv[2])=="--linkage-reduced";
            Assimp::Importer importer;
            const auto *scene=importer.ReadFile(argv[1],0);
            require(scene && scene->mNumMeshes==4 && scene->mNumAnimations==1,"mechanical clip scene changed");
            auto *clip=scene->mAnimations[0];
            require(std::string(clip->mName.C_Str())=="drive" && clip->mNumChannels==4 && clip->mTicksPerSecond>0 && std::abs(clip->mDuration/clip->mTicksPerSecond-1)<1e-6,"mechanical clip changed");
            double maxKey=0,maxBetween=0;
            unsigned keys=clip->mChannels[0]->mNumPositionKeys;
            require(reduced?keys>=2:adaptive?(keys>3 && keys<=97):keys==25,"mechanical sample count changed");
            unsigned totalKeys=0,intervals=reduced?128:(keys-1)*4;
            for(unsigned c=0;c<clip->mNumChannels;++c) {
                auto *channel=clip->mChannels[c];
                std::string name=channel->mNodeName.C_Str();
                require(name=="first/frame" || name=="first/crank" || name=="first/coupler" || name=="first/rocker","mechanical clip target changed");
                auto *node=scene->mRootNode->FindNode(name.c_str());
                auto *copy=scene->mRootNode->FindNode(("second/"+name.substr(6)).c_str());
                require(node && copy && node->mNumMeshes==1 && copy->mNumMeshes==1 && node->mMeshes[0]==copy->mMeshes[0],"mechanical instances lost sharing");
                require(reduced?(channel->mNumPositionKeys>=2 && channel->mNumRotationKeys>=2 && channel->mNumScalingKeys>=2):(channel->mNumPositionKeys==keys && channel->mNumRotationKeys==keys && channel->mNumScalingKeys==keys),"mechanical sample count changed");
                totalKeys+=channel->mNumPositionKeys+channel->mNumRotationKeys+channel->mNumScalingKeys;
                auto *mesh=scene->mMeshes[node->mMeshes[0]];
                for(unsigned sample=0;sample<=intervals;++sample) {
                    double time=sample/double(intervals),ticks=time*clip->mTicksPerSecond;
                    auto vectorAt=[&](const aiVectorKey *values,unsigned n) {
                        unsigned a=0;while(a+1<n && values[a+1].mTime<=ticks) ++a;
                        unsigned b=std::min(a+1,n-1);
                        float f=a==b?0.f:float((ticks-values[a].mTime)/(values[b].mTime-values[a].mTime));
                        return values[a].mValue*(1-f)+values[b].mValue*f;
                    };
                    unsigned a=0;
                    while(a+1<channel->mNumRotationKeys && channel->mRotationKeys[a+1].mTime<=ticks) ++a;
                    unsigned b=std::min(a+1,channel->mNumRotationKeys-1);
                    float fraction=a==b?0.f:float((ticks-channel->mRotationKeys[a].mTime)/(channel->mRotationKeys[b].mTime-channel->mRotationKeys[a].mTime));
                    aiQuaternion rotation;
                    aiQuaternion::Interpolate(rotation,channel->mRotationKeys[a].mValue,channel->mRotationKeys[b].mValue,fraction); rotation.Normalize();
                    auto position=vectorAt(channel->mPositionKeys,channel->mNumPositionKeys);
                    auto scale=vectorAt(channel->mScalingKeys,channel->mNumScalingKeys);
                    auto pose=worldMatrix(node->mParent)*aiMatrix4x4(scale,rotation,position);
                    // Independent circle intersection for the upper four-bar branch, in mm.
                    double angle=time*3.14159265358979323846/6;
                    double ax=-15*std::sin(angle),ay=15*std::cos(angle);
                    double dx=40-ax,dy=-ay,d=std::hypot(dx,dy);
                    double along=(1600-900+d*d)/(2*d),h=std::sqrt(1600-along*along);
                    double cx=ax+along*dx/d-h*dy/d,cy=ay+along*dy/d+h*dx/d;
                    double tx=0,ty=0,tz=0,phi=0;
                    if(name=="first/crank") {tz=4;phi=3.14159265358979323846/2+angle;}
                    if(name=="first/coupler") {tx=ax;ty=ay;tz=7;phi=std::atan2(cy-ay,cx-ax);}
                    if(name=="first/rocker") {tx=40;tz=4;phi=std::atan2(cy,cx-40);}
                    for(unsigned v=0;v<mesh->mNumVertices;++v) {
                        auto p=mesh->mVertices[v]; double x=p.x*1000,y=-p.z*1000,z=p.y*1000;
                        aiVector3D expected(float((x*std::cos(phi)-y*std::sin(phi)+tx)/1000),float((z+tz)/1000),float(-(x*std::sin(phi)+y*std::cos(phi)+ty)/1000));
                        double error=(pose*p-expected).Length();
                        if(sample%4==0) {maxKey=std::max(maxKey,error);require(error<(reduced?1.1e-5:1e-6),"mechanical key differs from analytic linkage");}
                        else {maxBetween=std::max(maxBetween,error);require(error<(reduced?1.1e-5:adaptive?1e-6:1e-5),"mechanical interpolated pose exceeds fixture limit");}
                    }
                }
            }
            if(reduced) require(totalKeys<396,"reduction did not remove keys");
            std::cout << "Mechanical linkage import passed: " << totalKeys << " property keys, " << intervals+1 << " sampled poses, analytic four-bar positions; max key " << maxKey << " m, between-key " << maxBetween << " m\n";
            return 0;
        }
        if (argc==3 && std::string(argv[2])=="--linkage-skin") {
            Assimp::Importer importer;
            const auto *scene=importer.ReadFile(argv[1],0);
            require(scene && scene->mNumMeshes==5 && scene->mNumAnimations==1,"skinned linkage mesh/clip count changed");
            auto *node=scene->mRootNode->FindNode("first/crank");
            auto *copy=scene->mRootNode->FindNode("second/crank");
            auto *joint=scene->mRootNode->FindNode("bone");
            require(node && copy && joint && node->mNumMeshes==1 && copy->mNumMeshes==1,"skinned linkage nodes missing");
            auto *mesh=scene->mMeshes[node->mMeshes[0]];
            auto *rigid=scene->mMeshes[copy->mMeshes[0]];
            require(mesh!=rigid && mesh->mNumBones==1 && rigid->mNumBones==0 && mesh->mNumVertices==rigid->mNumVertices,"skin leaked to repeated instance");
            auto *bone=mesh->mBones[0];
            require(std::string(bone->mName.C_Str())=="bone" && bone->mNumWeights==mesh->mNumVertices,"rigid bone weights missing");
            std::vector<bool> weighted(mesh->mNumVertices,false);
            for(unsigned i=0;i<bone->mNumWeights;++i) {
                auto weight=bone->mWeights[i];
                require(weight.mVertexId<weighted.size() && !weighted[weight.mVertexId] && std::abs(weight.mWeight-1)<1e-6,"invalid rigid weights");
                weighted[weight.mVertexId]=true;
            }
            auto *clip=scene->mAnimations[0];
            require(std::string(clip->mName.C_Str())=="lift" && clip->mNumChannels==1 && clip->mTicksPerSecond>0 && std::abs(clip->mDuration/clip->mTicksPerSecond-1)<1e-6,"skin clip changed");
            auto *channel=clip->mChannels[0];
            require(std::string(channel->mNodeName.C_Str())=="bone" && channel->mNumPositionKeys==2,"skin animation target changed");
            require(channel->mPositionKeys[0].mValue.Length()<1e-7 && (channel->mPositionKeys[1].mValue-aiVector3D(0,.01f,0)).Length()<1e-7,"skin lift axes/units changed");
            aiVector3D scale,position; aiQuaternion rotation;
            joint->mTransformation.Decompose(scale,rotation,position);
            auto rest=worldMatrix(node),copyRest=worldMatrix(copy),parent=worldMatrix(joint->mParent);
            double maxError=0;
            for(unsigned sample=0;sample<=8;++sample) {
                float t=sample/8.f;
                position=channel->mPositionKeys[0].mValue*(1-t)+channel->mPositionKeys[1].mValue*t;
                auto pose=parent*aiMatrix4x4(scale,rotation,position)*bone->mOffsetMatrix;
                for(unsigned v=0;v<mesh->mNumVertices;++v) {
                    auto error=pose*mesh->mVertices[v]-rest*mesh->mVertices[v]-aiVector3D(0,.01f*t,0);
                    require(error.Length()<1e-6,"skinned part differs from analytic lift");
                    maxError=std::max(maxError,double(error.Length()));
                    require((copyRest*rigid->mVertices[v]-rest*mesh->mVertices[v]-aiVector3D(.1f,0,0)).Length()<1e-6,"rigid copy differs from rest placement");
                }
            }
            std::cout << "Linkage skin import passed: separate rigid copy, complete weights, inverse bind, nine analytic poses; maximum error " << maxError << " m\n";
            return 0;
        }
        if (argc==3 && (std::string(argv[2])=="--linkage" || std::string(argv[2])=="--linkage-part")) {
            bool part=std::string(argv[2])=="--linkage-part";
            Assimp::Importer importer;
            const auto *scene=importer.ReadFile(argv[1],0);
            require(scene && scene->mNumMeshes==4 && scene->mNumAnimations==1,"linkage mesh/clip count changed");
            auto *first=scene->mRootNode->FindNode("first");
            auto *second=scene->mRootNode->FindNode("second");
            require(first && second && first->mNumChildren==4 && second->mNumChildren==4 && first->mNumMeshes==0 && second->mNumMeshes==0,"linkage groups changed");
            double maxError=0;
            for (auto name:{"frame","crank","coupler","rocker"}) {
                auto *a=scene->mRootNode->FindNode((std::string("first/")+name).c_str());
                auto *b=scene->mRootNode->FindNode((std::string("second/")+name).c_str());
                require(a && b && a->mParent==first && b->mParent==second && a->mNumMeshes==1 && b->mNumMeshes==1 && a->mMeshes[0]==b->mMeshes[0],"linkage instance identity changed");
                auto *mesh=scene->mMeshes[a->mMeshes[0]];
                auto aw=worldMatrix(a),bw=worldMatrix(b);
                for(unsigned i=0;i<mesh->mNumVertices;++i) {
                    auto error=bw*mesh->mVertices[i]-aw*mesh->mVertices[i]-aiVector3D(.1f,0,0);
                    maxError=std::max(maxError,double(error.Length()));
                    require(error.Length()<1e-6,"linkage repeated placement changed");
                }
                if(std::string(name)=="crank") {
                    aiColor4D colour;
                    require(scene->mMaterials[mesh->mMaterialIndex]->Get(AI_MATKEY_BASE_COLOR,colour)==AI_SUCCESS && colour.r>colour.g*2 && colour.r>colour.b*2,"assembly colour override lost");
                }
            }
            auto *clip=scene->mAnimations[0];
            require(std::string(clip->mName.C_Str())==(part?"part-lift":"lift") && clip->mNumChannels==1 && clip->mTicksPerSecond>0 && std::abs(clip->mDuration/clip->mTicksPerSecond-1)<1e-6,"linkage clip changed");
            auto *channel=clip->mChannels[0];
            require(std::string(channel->mNodeName.C_Str())==(part?"first/crank":"first") && channel->mNumPositionKeys==2,"linkage animation target changed");
            aiVector3D origin(0,part?.004f:0,0);
            require((channel->mPositionKeys[0].mValue-origin).Length()<1e-7 && (channel->mPositionKeys[1].mValue-origin-aiVector3D(0,.01f,0)).Length()<1e-7,"linkage clip units/axes changed");
            if(part) {
                auto *node=scene->mRootNode->FindNode("first/crank");
                auto *mesh=scene->mMeshes[node->mMeshes[0]];
                aiVector3D scale,position; aiQuaternion rotation;
                node->mTransformation.Decompose(scale,rotation,position);
                auto parent=worldMatrix(node->mParent),rest=worldMatrix(node);
                for(unsigned sample=0;sample<=8;++sample) {
                    float t=sample/8.f;
                    position=channel->mPositionKeys[0].mValue*(1-t)+channel->mPositionKeys[1].mValue*t;
                    auto posed=parent*aiMatrix4x4(scale,rotation,position);
                    for(unsigned v=0;v<mesh->mNumVertices;++v) {
                        auto error=posed*mesh->mVertices[v]-rest*mesh->mVertices[v]-aiVector3D(0,.01f*t,0);
                        require(error.Length()<1e-6,"imported part pose differs from analytic lift");
                        maxError=std::max(maxError,double(error.Length()));
                    }
                }
            }
            std::cout << "Linkage import passed: four shared meshes, eight placed bodies, per-part colour, " << (part?"part clip (nine sampled poses)":"group clip") << "; maximum error " << maxError << " m\n";
            return 0;
        }
        if (argc==3 && (std::string(argv[2])=="--plate-skin" || std::string(argv[2])=="--plate-flex" || std::string(argv[2])=="--plate-flex-narrow" || std::string(argv[2])=="--plate-flex-eased")) {
            Assimp::Importer importer;
            const auto *scene=importer.ReadFile(argv[1],0);
            verifyPlateSkin(scene,std::string(argv[2])!="--plate-skin",std::string(argv[2])=="--plate-flex-narrow"?.0075:.015,std::string(argv[2])=="--plate-flex-eased");
            return 0;
        }
        if (argc == 3 && std::string(argv[2]) == "--skin") {
            Assimp::Importer importer;
            const auto *scene = importer.ReadFile(argv[1], 0);
            require(scene && scene->mNumAnimations == 2, "skin fixture requires two clips");
            auto *ribbon = scene->mRootNode->FindNode("ribbon");
            auto *copy = scene->mRootNode->FindNode("unskinned");
            auto *base = scene->mRootNode->FindNode("base");
            auto *tip = scene->mRootNode->FindNode("tip");
            require(ribbon && copy && base && tip && tip->mParent == base, "skin hierarchy lost");
            require(ribbon->mNumMeshes == 1 && copy->mNumMeshes == 1 && ribbon->mMeshes[0] != copy->mMeshes[0], "skin mesh variant lost");
            auto *mesh = scene->mMeshes[ribbon->mMeshes[0]];
            require(mesh->mNumVertices == 6 && mesh->mNumFaces == 4 && mesh->mNumBones == 2, "skin geometry/bones changed");
            require(scene->mMeshes[copy->mMeshes[0]]->mNumBones == 0, "unskinned instance acquired bones");
            require(mesh->HasNormals() && mesh->HasTangentsAndBitangents() && mesh->HasTextureCoords(0), "skin attributes missing");
            aiString texture;
            require(scene->mMaterials[mesh->mMaterialIndex]->GetTexture(aiTextureType_BASE_COLOR,0,&texture) == AI_SUCCESS && scene->GetEmbeddedTexture(texture.C_Str()), "skin texture lost");
            float weights[2][6] = {};
            aiMatrix4x4 offsets[2]; bool seen[2] = {};
            for (unsigned b = 0; b < mesh->mNumBones; ++b) {
                auto *bone = mesh->mBones[b]; std::string name = bone->mName.C_Str();
                require(name == "base" || name == "tip", "unknown skin bone");
                unsigned index = name == "base" ? 0 : 1;
                require(!seen[index], "duplicate bone"); seen[index] = true; offsets[index] = bone->mOffsetMatrix;
                for (unsigned w = 0; w < bone->mNumWeights; ++w) {
                    auto entry = bone->mWeights[w];
                    require(entry.mVertexId < 6 && entry.mWeight > 0 && weights[index][entry.mVertexId] == 0, "invalid imported influence");
                    weights[index][entry.mVertexId] = entry.mWeight;
                }
            }
            for (unsigned i = 0; i < 6; ++i) {
                float expected = i < 2 ? 0.0f : i < 4 ? 0.5f : 1.0f;
                require(std::abs(weights[1][i]-expected)<1e-6 && std::abs(weights[0][i]-(1-expected))<1e-6, "imported weights changed");
            }
            double maximumError = 0; bool bendSeen = false, twistSeen = false;
            for (unsigned c = 0; c < scene->mNumAnimations; ++c) {
                auto *clip = scene->mAnimations[c]; std::string name = clip->mName.C_Str();
                bool bend = name == "bend";
                require(bend || name == "twist", "skin clip name changed");
                if (bend) { require(!bendSeen,"duplicate bend"); bendSeen=true; } else { require(!twistSeen,"duplicate twist"); twistSeen=true; }
                require(clip->mNumChannels == 1 && clip->mTicksPerSecond > 0 && std::abs(clip->mDuration/clip->mTicksPerSecond-1)<1e-6, "skin clip timing changed");
                auto *channel=clip->mChannels[0];
                require(std::string(channel->mNodeName.C_Str()) == "tip" && channel->mNumRotationKeys == 2, "skin rotation track changed");
                for (unsigned sample=0; sample<=8; ++sample) {
                    double time=sample/8.0;
                    auto &a=channel->mRotationKeys[0]; auto &b=channel->mRotationKeys[1];
                    require(b.mTime>a.mTime,"skin key times invalid");
                    aiVector3D scale, translation; aiQuaternion rotation;
                    tip->mTransformation.Decompose(scale,rotation,translation);
                    aiQuaternion::Interpolate(rotation,a.mValue,b.mValue,float((time*clip->mTicksPerSecond-a.mTime)/(b.mTime-a.mTime)));
                    rotation.Normalize();
                    auto baseWorld=worldMatrix(base);
                    aiMatrix4x4 tipWorld=baseWorld*aiMatrix4x4(scale,rotation,translation);
                    aiMatrix4x4 matrices[2]={baseWorld*offsets[0],tipWorld*offsets[1]};
                    double angle=time*3.14159265358979323846/2;
                    for (unsigned i=0; i<6; ++i) {
                        aiVector3D actual=(matrices[0]*mesh->mVertices[i])*weights[0][i]+(matrices[1]*mesh->mVertices[i])*weights[1][i];
                        double x=i%2 ? .05 : -.05, y=(i/2)*.5, weight=i<2 ? 0 : i<4 ? .5 : 1;
                        double tx=bend ? x*std::cos(angle)-(y-.5)*std::sin(angle) : x*std::cos(angle);
                        double ty=bend ? .5+x*std::sin(angle)+(y-.5)*std::cos(angle) : y;
                        double tz=bend ? 0 : -x*std::sin(angle);
                        double expected[3]={.1+x*(1-weight)+tx*weight, .3+tz*weight, -(.2+y*(1-weight)+ty*weight)};
                        double squared=0;
                        for(unsigned k=0;k<3;++k) squared+=(actual[k]-expected[k])*(actual[k]-expected[k]);
                        maximumError=std::max(maximumError,std::sqrt(squared));
                        require(std::sqrt(squared)<1e-6,"skinned deformation differs from analytic pose");
                    }
                }
            }
            require(bendSeen && twistSeen,"skin clip missing");
            std::cout << "Skin import passed: nonidentity binds, weights, two clips, unskinned copy; maximum sampled vertex error " << maximumError << " m\n";
            return 0;
        }
        if (argc == 3 && (std::string(argv[2]) == "--plate-motion" || std::string(argv[2]) == "--plate-scale")) {
            bool scaleMode = std::string(argv[2]) == "--plate-scale";
            Assimp::Importer importer;
            const auto *scene = importer.ReadFile(argv[1], 0);
            require(scene && scene->mNumAnimations == 2 && scene->mNumMeshes == 1, "expected plate with two clips");
            auto *plate = scene->mRootNode->FindNode("plate");
            auto *pivot = scene->mRootNode->FindNode("pivot");
            require(plate && pivot && plate->mParent == pivot, "animated hierarchy changed");
            double maximumError = 0;
            bool sawSlide = false, sawSwing = false;
            for (unsigned c = 0; c < scene->mNumAnimations; ++c) {
                auto *clip = scene->mAnimations[c];
                std::string name = clip->mName.C_Str();
                bool swing = name == "swing";
                bool snap = name == "snap";
                require(scaleMode ? snap || name == "resize" : swing || name == "slide", "unexpected clip name");
                if (swing || snap) { require(!sawSwing, "duplicate swing/snap"); sawSwing = true; }
                else { require(!sawSlide, "duplicate slide"); sawSlide = true; }
                require(clip->mTicksPerSecond > 0 && std::abs(clip->mDuration / clip->mTicksPerSecond - 2.0) < 1e-6 && clip->mNumChannels == 1, "clip timing or channel count changed");
                auto *channel = clip->mChannels[0];
                require(std::string(channel->mNodeName.C_Str()) == (swing ? "pivot" : "plate"), "clip target changed");
                require(scaleMode ? channel->mNumScalingKeys == (snap ? 3u : 2u) : swing ? channel->mNumRotationKeys == 2 : channel->mNumPositionKeys == 2, "animation keys lost");
                for (unsigned sample = 0; sample < 11; ++sample) {
                    double seconds = sample <= 8 ? sample * .25 : sample == 9 ? .999999 : 1.000001;
                    double ticks = seconds * clip->mTicksPerSecond;
                    aiVector3D scale, position; aiQuaternion rotation;
                    (swing ? pivot : plate)->mTransformation.Decompose(scale, rotation, position);
                    if (scaleMode) {
                        unsigned a = 0;
                        while (a + 1 < channel->mNumScalingKeys && channel->mScalingKeys[a+1].mTime <= ticks) ++a;
                        unsigned b = std::min(a+1, channel->mNumScalingKeys-1);
                        const auto &ka = channel->mScalingKeys[a], &kb = channel->mScalingKeys[b];
                        require(ka.mInterpolation == (snap ? aiAnimInterpolation_Step : aiAnimInterpolation_Linear), "scale interpolation mode changed");
                        float t = snap || a == b ? 0.0f : float((ticks-ka.mTime)/(kb.mTime-ka.mTime));
                        scale = ka.mValue*(1-t)+kb.mValue*t;
                    } else if (swing) {
                        auto &a = channel->mRotationKeys[0]; auto &b = channel->mRotationKeys[1];
                        require(b.mTime > a.mTime, "rotation times invalid");
                        aiQuaternion::Interpolate(rotation, a.mValue, b.mValue, float((ticks-a.mTime)/(b.mTime-a.mTime)));
                        rotation.Normalize();
                    } else {
                        auto &a = channel->mPositionKeys[0]; auto &b = channel->mPositionKeys[1];
                        require(b.mTime > a.mTime, "position times invalid");
                        float t = float((ticks-a.mTime)/(b.mTime-a.mTime));
                        position = a.mValue * (1-t) + b.mValue * t;
                    }
                    aiMatrix4x4 animated(scale, rotation, position);
                    aiMatrix4x4 parentWorld = pivot->mParent ? worldMatrix(pivot->mParent) : aiMatrix4x4();
                    auto world = parentWorld * (swing ? animated : pivot->mTransformation) * (swing ? plate->mTransformation : animated);
                    aiVector3D point = world * aiVector3D(.01f, .0015f, -.005f);
                    double angle = seconds * 3.14159265358979323846 / 4.0;
                    double x = swing ? .06*std::cos(angle)-.005*std::sin(angle) : .06+.05*seconds;
                    double z = swing ? -(.1+.06*std::sin(angle)+.005*std::cos(angle)) : -.105;
                    double y = .0015;
                    if (scaleMode) {
                        double sx = snap ? (seconds >= 1 && seconds < 2 ? -2 : 1) : 1+seconds*.5;
                        double sy = snap ? (seconds >= 1 && seconds < 2 ? 3 : 1) : 1+seconds;
                        double sz = snap ? (seconds >= 1 && seconds < 2 ? 4 : 1) : 1+seconds*1.5;
                        x = .05+.01*sx; y = .0015*sz; z = -(.1+.005*sy);
                        require((world.Determinant() < 0) == (sx < 0), "animated reflection changed");
                    }
                    double error = std::sqrt((point.x-x)*(point.x-x)+(point.y-y)*(point.y-y)+(point.z-z)*(point.z-z));
                    maximumError = std::max(maximumError,error);
                    require(error < 1e-6, "sampled animation differs from analytic pose");
                }
            }
            require(sawSlide && sawSwing, "clip missing");
            std::cout << "Authored animation import passed: two clips, eleven samples each; maximum position error " << maximumError << " m\n";
            return 0;
        }
        if (argc == 3 && std::string(argv[2]) == "--plate-scene") {
            Assimp::Importer importer;
            const auto *scene = importer.ReadFile(argv[1], 0);
            require(scene && scene->mNumMeshes == 1, "plate scene must share one mesh");
            auto *mirror = scene->mRootNode->FindNode("mirror");
            auto *copy = scene->mRootNode->FindNode("copy");
            auto *assembly = scene->mRootNode->FindNode("assembly");
            require(mirror && copy && assembly && mirror->mParent == assembly && copy->mParent == assembly, "plate hierarchy changed");
            require(mirror->mNumMeshes == 1 && copy->mNumMeshes == 1 && mirror->mMeshes[0] == copy->mMeshes[0], "plate instances do not share geometry");
            const float minimum[2][3] = {{-.045f, 0, -.19f}, {-.015f, 0, -.07f}};
            const float maximum[2][3] = {{.045f, .012f, -.11f}, {.015f, .003f, -.03f}};
            for (unsigned instance = 0; instance < 2; ++instance) {
                auto *node = instance ? copy : mirror;
                auto world = worldMatrix(node);
                require(instance ? world.Determinant() > 0 : world.Determinant() < 0, "plate reflection changed");
                auto *mesh = scene->mMeshes[node->mMeshes[0]];
                require(mesh->HasNormals() && mesh->HasTangentsAndBitangents() && mesh->HasTextureCoords(0), "plate attributes missing");
                float low[3] = {INFINITY, INFINITY, INFINITY}, high[3] = {-INFINITY, -INFINITY, -INFINITY};
                for (unsigned i = 0; i < mesh->mNumVertices; ++i) {
                    auto p = world * mesh->mVertices[i];
                    for (unsigned k = 0; k < 3; ++k) { low[k] = std::min(low[k], p[k]); high[k] = std::max(high[k], p[k]); }
                }
                for (unsigned k = 0; k < 3; ++k) require(std::abs(low[k] - minimum[instance][k]) < 1e-6 && std::abs(high[k] - maximum[instance][k]) < 1e-6, "plate analytic world bounds changed");
            }
            std::cout << "Authored plate scene import passed: hierarchy, shared geometry, reflection, analytic metre bounds and attributes\n";
            return 0;
        }
        require(argc == 2 || (argc == 3 && (std::string(argv[2]) == "--rich" || std::string(argv[2]) == "--scene" || std::string(argv[2]) == "--baked")), "usage: gcad_glb_probe prop.glb [--rich|--scene|--baked]");
        const bool sceneMode = argc == 3 && std::string(argv[2]) == "--scene";
        const bool rich = argc == 3 && std::string(argv[2]) == "--rich";
        const bool baked = argc == 3 && std::string(argv[2]) == "--baked";
        Assimp::Importer importer;
        const auto *scene = importer.ReadFile(argv[1], 0);
        if (!scene) throw std::runtime_error(importer.GetErrorString());
        if (sceneMode) { verifyScene(scene); return 0; }
        require(scene->mNumMeshes == 1, "expected one mesh");
        const auto *mesh = scene->mMeshes[0];
        require(mesh->mNumVertices == 24 && mesh->mNumFaces == 12, "hard edges or geometry changed");
        require(mesh->HasNormals() && mesh->HasTangentsAndBitangents() && mesh->HasTextureCoords(0), "vertex attributes missing");
        float low[3] = {INFINITY, INFINITY, INFINITY};
        float high[3] = {-INFINITY, -INFINITY, -INFINITY};
        for (unsigned i = 0; i < mesh->mNumVertices; ++i) {
            for (unsigned k = 0; k < 3; ++k) {
                low[k] = std::min(low[k], mesh->mVertices[i][k]);
                high[k] = std::max(high[k], mesh->mVertices[i][k]);
            }
            const auto &normal = mesh->mNormals[i];
            const auto &tangent = mesh->mTangents[i];
            require(std::abs(normal.SquareLength() - 1) < 1e-5 &&
                std::abs(tangent.SquareLength() - 1) < 1e-5 && std::abs(normal * tangent) < 1e-5,
                "normal/tangent basis invalid");
        }
        const float expectedLow[3] = {-0.5f, 0, -0.25f};
        const float expectedHigh[3] = {0.5f, 0.25f, 0.25f};
        for (unsigned k = 0; k < 3; ++k) {
            require(std::abs(low[k] - expectedLow[k]) < 1e-6 && std::abs(high[k] - expectedHigh[k]) < 1e-6, "metre units or Y-up axes incorrect");
        }
        for (unsigned i = 0; i < mesh->mNumFaces; ++i) {
            const auto &face = mesh->mFaces[i];
            require(face.mNumIndices == 3, "non-triangle face");
            auto a = face.mIndices[0], b = face.mIndices[1], c = face.mIndices[2];
            auto geometric = (mesh->mVertices[b] - mesh->mVertices[a]) ^ (mesh->mVertices[c] - mesh->mVertices[a]);
            require(geometric * mesh->mNormals[a] > 0, "winding and normals disagree");
        }
        require(scene->mNumTextures == (rich ? 5u : 2u), "embedded PNG image count changed");
        for (unsigned i = 0; i < scene->mNumTextures; ++i) {
            auto *texture = scene->mTextures[i];
            require(texture->mHeight == 0 && texture->mWidth > 8, "expected compressed image");
            const auto *bytes = reinterpret_cast<const unsigned char *>(texture->pcData);
            require(bytes[0] == 137 && bytes[1] == 'P' && bytes[2] == 'N' && bytes[3] == 'G', "embedded image is not PNG");
            if (baked) {
                require(texture->mWidth >= 24, "truncated PNG header");
                auto be32 = [&](unsigned offset) { return (unsigned(bytes[offset]) << 24) | (unsigned(bytes[offset+1]) << 16) | (unsigned(bytes[offset+2]) << 8) | unsigned(bytes[offset+3]); };
                require(be32(16) == 256 && be32(20) == 256, "baked tile dimensions changed");
            }
        }
        require(mesh->mMaterialIndex < scene->mNumMaterials, "material reference invalid");
        auto *material = scene->mMaterials[mesh->mMaterialIndex];
        float metallic = -1, roughness = -1;
        require(material->Get(AI_MATKEY_METALLIC_FACTOR, metallic) == AI_SUCCESS &&
            material->Get(AI_MATKEY_ROUGHNESS_FACTOR, roughness) == AI_SUCCESS, "PBR factors missing");
        require(std::abs(metallic - 0.75f) < 1e-6 && std::abs(roughness - 0.8f) < 1e-6, "PBR factors changed");
        aiString path;
        require(material->GetTexture(aiTextureType_BASE_COLOR, 0, &path) == AI_SUCCESS && scene->GetEmbeddedTexture(path.C_Str()), "base colour texture not connected");
        if (!baked) require(material->GetTexture(aiTextureType_METALNESS, 0, &path) == AI_SUCCESS && scene->GetEmbeddedTexture(path.C_Str()), "metal/rough texture not connected");
        if (baked) {
            require(material->GetTexture(aiTextureType_NORMALS, 0, &path) == AI_SUCCESS && scene->GetEmbeddedTexture(path.C_Str()), "baked normal texture not connected");
            aiColor4D colour;
            require(material->Get(AI_MATKEY_BASE_COLOR, colour) == AI_SUCCESS, "baked colour missing");
            const float expected[3] = {0.21404114f, 0.05087609f, 0.52252155f};
            require(std::abs(colour.r - expected[0]) < 1e-6 && std::abs(colour.g - expected[1]) < 1e-6 && std::abs(colour.b - expected[2]) < 1e-6, "baked linear finish tint changed");
            for (unsigned i = 0; i < mesh->mNumVertices; ++i) {
                const auto &p = mesh->mVertices[i]; const auto &n = mesh->mNormals[i];
                const float cad[3] = {p.x * 1000, -p.z * 1000, p.y * 1000};
                const float normal[3] = {n.x, -n.z, n.y};
                unsigned axis = 0;
                for (unsigned k = 1; k < 3; ++k) if (std::abs(normal[k]) > std::abs(normal[axis])) axis = k;
                const float u = -cad[(axis + 2) % 3] / 250;
                const float v = cad[(axis + 1) % 3] / 250 * (normal[axis] < 0 ? -1 : 1);
                // Assimp converts glTF's top-left V to its own bottom-left convention.
                require(std::abs(mesh->mTextureCoords[0][i].x - u) < 1e-5 && std::abs(mesh->mTextureCoords[0][i].y - (1-v)) < 1e-5, "baked tile size or 90-degree UV rotation changed");
            }
        }
        if (rich) {
            for (auto type : {aiTextureType_NORMALS, aiTextureType_LIGHTMAP, aiTextureType_EMISSIVE}) {
                require(material->GetTexture(type, 0, &path) == AI_SUCCESS && scene->GetEmbeddedTexture(path.C_Str()), "extended texture not connected");
            }
            float scale = 0, strength = 0, cutoff = 0;
            int twoSided = 0;
            aiString alpha;
            aiColor3D emission;
            aiColor4D base;
            require(material->Get(AI_MATKEY_GLTF_TEXTURE_SCALE(aiTextureType_NORMALS, 0), scale) == AI_SUCCESS && std::abs(scale - 0.6f) < 1e-6, "normal scale changed");
            require(material->Get(AI_MATKEY_GLTF_TEXTURE_STRENGTH(aiTextureType_LIGHTMAP, 0), strength) == AI_SUCCESS && std::abs(strength - 0.4f) < 1e-6, "occlusion strength changed");
            require(material->Get(AI_MATKEY_GLTF_ALPHAMODE, alpha) == AI_SUCCESS && std::string(alpha.C_Str()) == "MASK", "alpha mode changed");
            require(material->Get(AI_MATKEY_GLTF_ALPHACUTOFF, cutoff) == AI_SUCCESS && std::abs(cutoff - 0.35f) < 1e-6, "alpha cutoff changed");
            require(material->Get(AI_MATKEY_TWOSIDED, twoSided) == AI_SUCCESS && twoSided == 1, "double sided flag lost");
            require(material->Get(AI_MATKEY_COLOR_EMISSIVE, emission) == AI_SUCCESS && std::abs(emission.r - 0.2f) < 1e-6 && std::abs(emission.g - 0.4f) < 1e-6 && std::abs(emission.b - 0.6f) < 1e-6, "emission changed");
            require(material->Get(AI_MATKEY_BASE_COLOR, base) == AI_SUCCESS && std::abs(base.a - 0.65f) < 1e-6, "base alpha changed");
            int wrapU = -1, wrapV = -1, mag = -1, min = -1;
            require(material->Get(AI_MATKEY_MAPPINGMODE_U(aiTextureType_BASE_COLOR, 0), wrapU) == AI_SUCCESS && wrapU == aiTextureMapMode_Clamp, "U wrapping changed");
            require(material->Get(AI_MATKEY_MAPPINGMODE_V(aiTextureType_BASE_COLOR, 0), wrapV) == AI_SUCCESS && wrapV == aiTextureMapMode_Mirror, "V wrapping changed");
            require(material->Get(AI_MATKEY_GLTF_MAPPINGFILTER_MAG(aiTextureType_BASE_COLOR, 0), mag) == AI_SUCCESS && mag == 9728, "magnification changed");
            require(material->Get(AI_MATKEY_GLTF_MAPPINGFILTER_MIN(aiTextureType_BASE_COLOR, 0), min) == AI_SUCCESS && min == 9728, "minification changed");
        }
        std::cout << "GLB independent import passed: metre/Y-up bounds, 24 vertices, 12 triangles, winding, tangent basis, PBR and " << scene->mNumTextures << " embedded PNGs" << (rich ? "; normal/occlusion/emission, alpha mask, double-sided and sampler settings" : "") << '\n';
        return 0;
    } catch (const std::exception &error) {
        std::cerr << "GLB probe failed: " << error.what() << '\n';
        return 1;
    }
}
