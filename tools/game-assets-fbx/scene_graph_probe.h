// Validate all nodes, including unreachable ones, before importer recursion.
// Audit ceilings are deliberately separate from a production asset budget.
static void verifySourceSceneGraph(const rapidjson::Document& doc) {
    if(!doc.HasMember("nodes"))return; // Later scene-schema checks report absence.
    require(doc["nodes"].IsArray(),"source nodes must be an array");
    const auto& nodes=doc["nodes"];const size_t count=nodes.Size();
    require(count<=65536,"source node count exceeds audit limit");
    std::vector<size_t> parent(count,count),depth(count,1),queue;
    std::vector<std::vector<size_t>> children(count);
    for(size_t i=0;i<count;++i) {
        const auto& node=nodes[rapidjson::SizeType(i)];require(node.IsObject(),"source node must be an object");
        if(!node.HasMember("children"))continue;
        require(node["children"].IsArray(),"source children must be an array");
        std::set<size_t> unique;
        for(const auto& child:node["children"].GetArray()) {
            require(child.IsUint() && child.GetUint()<count,"source child reference invalid");
            size_t index=child.GetUint();
            require(unique.insert(index).second,"duplicate source child reference");
            require(parent[index]==count,"source node has multiple parents");
            parent[index]=i;children[i].push_back(index);
        }
    }
    for(size_t i=0;i<count;++i)if(parent[i]==count)queue.push_back(i);
    for(size_t head=0;head<queue.size();++head) {
        size_t node=queue[head];require(depth[node]<=128,"source hierarchy exceeds audit depth limit");
        for(size_t child:children[node]){depth[child]=depth[node]+1;queue.push_back(child);}
    }
    require(queue.size()==count,"source node graph contains a cycle");
    if(doc.HasMember("scenes")) {
        require(doc["scenes"].IsArray(),"source scenes must be an array");
        for(const auto& scene:doc["scenes"].GetArray()) {
            require(scene.IsObject(),"source scene must be an object");
            if(!scene.HasMember("nodes"))continue;
            require(scene["nodes"].IsArray(),"source scene roots must be an array");
            std::set<size_t> roots;
            for(const auto& root:scene["nodes"].GetArray()) {
                require(root.IsUint() && root.GetUint()<count,"source scene root invalid");
                auto index=root.GetUint();require(roots.insert(index).second,"duplicate source scene root");
                require(parent[index]==count,"source scene root has a parent");
            }
        }
    }
    if(doc.HasMember("scene"))require(doc["scene"].IsUint() && doc.HasMember("scenes") &&
        doc["scenes"].IsArray() && doc["scene"].GetUint()<doc["scenes"].Size(),"source default scene invalid");
}
