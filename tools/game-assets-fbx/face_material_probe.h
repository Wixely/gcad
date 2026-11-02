// Independent paired import for base.end -> red/roughness 0.2 material exports.
// Assumes the selected face is the mesh's maximum glTF Y (CAD Z) plane.
#include <map>
#include <set>
#include <cstring>
#include "face_uv_probe.h"

static void verifyFaceMaterial(const char* reference, const char* candidate, const char* target, bool roughnessOnly=false, bool uvOnly=false, bool weightsOnly=false) {
    Assimp::Importer imports[2];
    const aiScene* scenes[2] = {imports[0].ReadFile(reference,0), imports[1].ReadFile(candidate,0)};
    require(scenes[0] && scenes[1], "face material import failed");
    std::map<std::string,const aiNode*> nodes[2];
    for(unsigned s=0;s<2;++s) {
        std::vector<const aiNode*> pending{scenes[s]->mRootNode};
        while(!pending.empty()) {
            auto* n=pending.back();pending.pop_back();
            require(nodes[s].emplace(n->mName.C_Str(),n).second,"duplicate imported node name");
            for(unsigned i=0;i<n->mNumChildren;++i)pending.push_back(n->mChildren[i]);
        }
    }
    require(nodes[0].size()==nodes[1].size() && nodes[0].count(target),"node set changed or target absent");
    for(const auto& entry:nodes[0]) {
        require(nodes[1].count(entry.first),"node disappeared");
        auto* a=entry.second;auto* b=nodes[1].at(entry.first);
        require(a->mTransformation==b->mTransformation,"node transform changed");
        require(bool(a->mParent)==bool(b->mParent) && (!a->mParent || a->mParent->mName==b->mParent->mName),"node hierarchy changed");
    }
    require(scenes[0]->mNumAnimations==scenes[1]->mNumAnimations,"clip count changed");
    for(unsigned i=0;i<scenes[0]->mNumAnimations;++i) {
        const auto* a=scenes[0]->mAnimations[i]; const auto* b=scenes[1]->mAnimations[i];
        require(a->mName==b->mName && a->mDuration==b->mDuration && a->mTicksPerSecond==b->mTicksPerSecond && a->mNumChannels==b->mNumChannels,"clip metadata changed");
        for(unsigned c=0;c<a->mNumChannels;++c) {
            auto* x=a->mChannels[c];auto* y=b->mChannels[c];
            require(x->mNodeName==y->mNodeName && x->mNumPositionKeys==y->mNumPositionKeys && x->mNumRotationKeys==y->mNumRotationKeys && x->mNumScalingKeys==y->mNumScalingKeys,"clip channels changed");
            for(unsigned k=0;k<x->mNumPositionKeys;++k)require(x->mPositionKeys[k].mTime==y->mPositionKeys[k].mTime && x->mPositionKeys[k].mValue==y->mPositionKeys[k].mValue,"translation key changed");
            for(unsigned k=0;k<x->mNumRotationKeys;++k)require(x->mRotationKeys[k].mTime==y->mRotationKeys[k].mTime && x->mRotationKeys[k].mValue==y->mRotationKeys[k].mValue,"rotation key changed");
            for(unsigned k=0;k<x->mNumScalingKeys;++k)require(x->mScalingKeys[k].mTime==y->mScalingKeys[k].mTime && x->mScalingKeys[k].mValue==y->mScalingKeys[k].mValue,"scale key changed");
        }
    }
    size_t triangles=0, selected=0, posedCorners=0, changedPosedCorners=0;
    for(const auto& entry:nodes[0]) {
        auto* a=entry.second;auto* b=nodes[1].at(entry.first);
        const bool changed=entry.first==target;
        if(!a->mNumMeshes){require(!b->mNumMeshes,"non-mesh node changed");continue;}
        if(uvOnly || weightsOnly)require(a->mNumMeshes==b->mNumMeshes,"face edit changed primitive count");
        else if(roughnessOnly)require(a->mNumMeshes==b->mNumMeshes && (!changed || a->mNumMeshes>=2),"roughness fixture must retain multiple primitives");
        else require(a->mNumMeshes==1 && b->mNumMeshes==(changed?2u:1u),"expected one original and two assigned primitives");
        const auto* original=scenes[0]->mMeshes[a->mMeshes[0]];
        float top=-INFINITY;for(unsigned v=0;v<original->mNumVertices;++v)top=std::max(top,original->mVertices[v].y);
        std::multiset<std::vector<float>> signatures[2];
        for(unsigned s=0;s<2;++s) {
            auto* node=s?b:a;
            for(unsigned p=0;p<node->mNumMeshes;++p) {
                const auto* mesh=scenes[s]->mMeshes[node->mMeshes[p]];
                require(mesh->HasNormals() && mesh->HasTangentsAndBitangents() && mesh->HasTextureCoords(0),"face material attributes missing");
                std::map<std::string,const aiBone*> bones;
                for(unsigned j=0;j<mesh->mNumBones;++j)require(bones.emplace(mesh->mBones[j]->mName.C_Str(),mesh->mBones[j]).second,"duplicate bone");
                std::vector<std::vector<float>> weights(mesh->mNumVertices,std::vector<float>(bones.size()));
                unsigned joint=0;
                for(const auto& bone:bones) {
                    require(nodes[s].count(bone.first),"bone node missing");
                    for(unsigned w=0;w<bone.second->mNumWeights;++w) {
                        auto weight=bone.second->mWeights[w];require(weight.mVertexId<mesh->mNumVertices,"weight vertex out of range");
                        weights[weight.mVertexId][joint]+=weight.mWeight;
                    }
                    ++joint;
                }
                const auto* material=scenes[s]->mMaterials[mesh->mMaterialIndex];
                const auto* base=scenes[0]->mMaterials[(roughnessOnly || uvOnly || weightsOnly)?scenes[0]->mMeshes[a->mMeshes[p]]->mMaterialIndex:original->mMaterialIndex];
                for(auto type:{aiTextureType_BASE_COLOR,aiTextureType_NORMALS,aiTextureType_METALNESS,aiTextureType_DIFFUSE_ROUGHNESS,aiTextureType_LIGHTMAP,aiTextureType_EMISSIVE}) {
                    require(material->GetTextureCount(type)==base->GetTextureCount(type),"texture count changed");
                    for(unsigned t=0;t<base->GetTextureCount(type);++t) {
                        aiString x,y;require(base->GetTexture(type,t,&x)==AI_SUCCESS && material->GetTexture(type,t,&y)==AI_SUCCESS,"texture disconnected");
                        const auto* tx=scenes[0]->GetEmbeddedTexture(x.C_Str());const auto* ty=scenes[s]->GetEmbeddedTexture(y.C_Str());
                        require(tx && ty && tx->mHeight==0 && ty->mHeight==0 && tx->mWidth==ty->mWidth && !std::memcmp(tx->pcData,ty->pcData,tx->mWidth),"embedded texture changed");
                    }
                }
                for(unsigned f=0;f<mesh->mNumFaces;++f) {
                    const auto& face=mesh->mFaces[f];require(face.mNumIndices==3,"non-triangle face");
                    bool end=true;for(unsigned k=0;k<3;++k)end &= std::abs(mesh->mVertices[face.mIndices[k]].y-top)<1e-7f;
                    const bool red=changed && end;
                    aiColor4D colour,expected;float rough=0,expectedRough=0,metal=0,expectedMetal=0;
                    require(material->Get(AI_MATKEY_BASE_COLOR,colour)==AI_SUCCESS && base->Get(AI_MATKEY_BASE_COLOR,expected)==AI_SUCCESS,"base colour missing");
                    require(material->Get(AI_MATKEY_ROUGHNESS_FACTOR,rough)==AI_SUCCESS && base->Get(AI_MATKEY_ROUGHNESS_FACTOR,expectedRough)==AI_SUCCESS,"roughness missing");
                    require(material->Get(AI_MATKEY_METALLIC_FACTOR,metal)==AI_SUCCESS && base->Get(AI_MATKEY_METALLIC_FACTOR,expectedMetal)==AI_SUCCESS,"metallic missing");
                    if(s && (uvOnly || weightsOnly)){if(red)++selected;}
                    else if(s && roughnessOnly && changed){expectedRough=.25f;++selected;}
                    else if(s && !roughnessOnly && red){expected=aiColor4D(1,0,0,1);expectedRough=.2f;++selected;}
                    require(colour==expected && rough==expectedRough && metal==expectedMetal,"wrong face material factors");
                    std::vector<float> key{red?1.f:0.f};
                    auto append=[&](aiVector3D v){require(std::isfinite(v.x)&&std::isfinite(v.y)&&std::isfinite(v.z),"nonfinite attribute");key.insert(key.end(),{v.x,v.y,v.z});};
                    for(unsigned k=0;k<3;++k) {
                        auto v=face.mIndices[k];append(mesh->mVertices[v]);append(mesh->mNormals[v]);
                        auto cornerWeights=weights[v];
                        if(weightsOnly && red) {
                            require(bones.size()==2 && bones.count("base") && bones.count("tip"),"weight fixture needs base/tip palette");
                            // Reference-side oracle replaces only selected corner weights.
                            // Candidate-side values are always the independently imported data.
                            if(!s)cornerWeights={.25f,.75f};
                        }
                        if(s && uvOnly && red) {
                            const auto* prior=scenes[0]->mMeshes[a->mMeshes[p]];
                            verifyFaceUvCorner(prior,mesh,f,k);
                            auto old=prior->mFaces[f].mIndices[k];
                            append(prior->mTangents[old]);append(prior->mBitangents[old]);append(prior->mTextureCoords[0][old]);
                        } else {append(mesh->mTangents[v]);append(mesh->mBitangents[v]);append(mesh->mTextureCoords[0][v]);}
                        key.push_back(float(bones.size()));joint=0;
                        for(const auto& bone:bones) {
                            key.push_back(float(std::distance(nodes[s].begin(),nodes[s].find(bone.first))));
                            key.push_back(cornerWeights[joint++]);
                            for(unsigned row=0;row<4;++row)for(unsigned col=0;col<4;++col)key.push_back(bone.second->mOffsetMatrix[row][col]);
                        }
                        if(!bones.empty())for(unsigned sample=0;sample<17;++sample) {
                            aiVector3D point, originalPoint; joint=0;
                            for(const auto& bone:bones) {
                                float angle=float(sample)*.04f*float(joint+1);
                                aiQuaternion rotation(aiVector3D(0,1,0),angle);
                                aiMatrix4x4 pose(aiVector3D(1+.01f*sample,1,1),rotation,aiVector3D(0,.001f*sample,0));
                                auto transformed=worldMatrix(nodes[s].at(bone.first))*pose*bone.second->mOffsetMatrix*mesh->mVertices[v];
                                originalPoint+=transformed*weights[v][joint];
                                point+=transformed*cornerWeights[joint++];
                            }
                            if(weightsOnly && !s && red && (point-originalPoint).Length()>1e-7f)++changedPosedCorners;
                            append(point);if(s)++posedCorners;
                        }
                    }
                    signatures[s].insert(std::move(key));if(s)++triangles;
                }
            }
        }
        require(signatures[0]==signatures[1],"triangle multiplicity/winding, attributes, weights, bind matrices or sampled deformation changed");
    }
    require(selected>0,"no selected end face");
    if(weightsOnly)require(changedPosedCorners>0,"weight fixture does not exercise changed deformation");
    std::cout<<(weightsOnly?"Face weights":uvOnly?"Face UV":"Face material")<<" independent import passed: "<<triangles<<" triangles, "<<selected<<" selected, "<<posedCorners<<" synthetic posed corners, "<<changedPosedCorners<<" changed expected positions; hierarchy, clips, PBR, embedded maps and unchanged tuples preserved\n";
}
