source('./scripts-ub/postgres.R')
library('igraph')

args = commandArgs(trailingOnly=TRUE)
if(length(args) == 0) {
	    stop("il faut indiquer le schéma")
}

schema <- args[1]

print(paste('"', schema, '"'))
buildGraphFromGraphParentAndNodes <- function(graphName, description, notebookUrl, graphParent, nodes, modularity) {
    # Construction du graph
    sendQuery(sprintf("INSERT INTO \"%s\".graph(name, description, directed, notebook_url, graph_parent, modularity)
        SELECT '%s' AS name, '%s' AS description, directed, '%s' AS notebook_url, '%s' AS graph_parent, %f AS modularity
        FROM \"%s\".graph
        WHERE name = '%s'",
        schema, graphName, description, notebookUrl, graphParent, modularity, schema, graphParent))
    
    # Construction des noeuds
    nodes$graph_name <- graphName
    writeTable(name = c(schema, "node"), value = nodes, overwrite = FALSE, append = TRUE, row.names = FALSE)
    
    # Construction des liens
    sendQuery(sprintf("INSERT INTO \"%s\".link(graph_name, node_out, node_in, weight) 
        SELECT '%s' AS graph_name, node_out, node_in, weight
        FROM \"%s\".link
        WHERE node_in IN (SELECT id FROM \"%s\".node WHERE graph_name = '%s')
            AND node_out IN (SELECT id FROM \"%s\".node WHERE graph_name = '%s')
            AND graph_name = '%s'", 
        schema, graphName, schema, schema, graphName, schema, graphName, graphParent))
}

getMaxComposanteConnexe <- function(graph) {
    cl <- clusters(graph)
    maxSizeIndex <- which(cl$csize == max(cl$csize))
    for (group in groups(cl)[maxSizeIndex]) {
        newGraph <- induced_subgraph(graph, group)
    }
    return(newGraph)
}

name <- "user_user_retweet"

description <- "Graphe dont les noeuds sont les users, 
et le liens sont les retweets qu''un user fait des tweets d''un autre user. 
Le poids des liens représente le nombre de retweets."

linksQuery <- sprintf("SELECT t1.user_id AS node_out, t2.user_id AS node_in, COUNT(*) AS weight
    FROM \"%s\".tweet t1
        INNER JOIN \"%s\".retweet rt ON t1.id = rt.tweet_id
        INNER JOIN \"%s\".tweet t2 ON t2.id = rt.retweeted_tweet_id
    WHERE t1.user_id <> t2.user_id
    GROUP BY node_out, node_in
    HAVING COUNT(*) > 5", schema, schema, schema)

buildGraph(schema, name, description, linksQuery)

name <- "user_user_mention"

description <- "Graphe dont les noeuds sont les users, 
et le liens sont les mentions qu''un user fait d''un autre user. 
Le poids des liens représente le nombre de mentions."

linksQuery <- sprintf("SELECT t.user_id AS node_out, m.user_id AS node_in, COUNT(*) AS weight
    FROM \"%s\".tweet t
        INNER JOIN (SELECT DISTINCT tweet_id, user_id FROM \"%s\".tweet_user_mention) AS m ON t.id = m.tweet_id
    WHERE t.user_id <> m.user_id
    GROUP BY node_out, node_in
    HAVING COUNT(*) > 5", schema, schema)

buildGraph(schema, name, description, linksQuery)

name <- "user_user_quote"

description <- "Graphe dont les noeuds sont les users, 
et le liens sont les quotes qu''un user fait des tweets d''un autre user. 
Le poids des liens représente le nombre de quotes"

linksQuery <- sprintf("SELECT t1.user_id AS node_out, t2.user_id AS node_in, COUNT(*) AS weight
    FROM \"%s\".tweet t1
        INNER JOIN \"%s\".quote q ON t1.id = q.tweet_id
        INNER JOIN \"%s\".tweet t2 ON t2.id = q.quoted_tweet_id
    WHERE t1.user_id <> t2.user_id
    GROUP BY node_out, node_in
    HAVING COUNT(*) > 5", schema, schema, schema)

buildGraph(schema, name, description, linksQuery)

name <- "hashtag_hashtag"

description <- "Graphe dont les noeuds sont les hashtags, 
et le liens sont les coocurrences entre les hashtags. 
Le poids des liens représente le nombre de coocurrences."

linksQuery <- sprintf("SELECT ht1.hashtag AS node_out, ht2.hashtag AS node_in, COUNT(*) AS weight
    FROM (SELECT DISTINCT tweet_id, hashtag FROM \"%s\".tweet_hashtag) ht1
        INNER JOIN (SELECT DISTINCT tweet_id, hashtag FROM \"%s\".tweet_hashtag) ht2 ON ht1.tweet_id = ht2.tweet_id AND ht1.hashtag > ht2.hashtag
    GROUP BY node_out, node_in
    HAVING COUNT(*) > 5", schema, schema)

buildGraph(schema, name, description, linksQuery, FALSE)

name <- "user_hashtag"

description <- "Graphe dont les noeuds sont les users et hashtags, 
et le liens sont les utilisations des hashtags par les users. 
Le poids des liens représente le nombre d''utilisations."

linksQuery <- sprintf("SELECT '@'||t.user_id AS node_out, '#'||ht.hashtag AS node_in, COUNT(*) AS weight
    FROM \"%s\".tweet t
        INNER JOIN (SELECT DISTINCT tweet_id, hashtag FROM \"%s\".tweet_hashtag) ht ON t.id = ht.tweet_id
    GROUP BY node_out, node_in
    HAVING COUNT(*) > 5", schema, schema)

buildGraph(schema, name, description, linksQuery)

computeDegreeCentrality <- function(graphName) {
    print(sprintf("Calcul de la centralité de degrés pour le graphe %s.", graphName))
    # Construction du graphe
    graph <- getGraph(schema, graphName)

    if (!is.null(graph)) {
        # Calcul des centralités
        inDegree <- as.data.frame(degree(graph, mode = "in"))
        outDegree <- as.data.frame(degree(graph, mode = "out"))
        inOutDegree <- as.data.frame(degree(graph, mode = "total"))

        # Ajout du node_id en 1ere place du dataframe
        inDegree <- cbind(rownames(inDegree), inDegree)
        colnames(inDegree) <- c("node_id", "value")
        inDegree$graph_name = graphName
        inDegree$name = "degree_in_centrality"
        outDegree <- cbind(rownames(outDegree), outDegree)
        colnames(outDegree) <- c("node_id", "value")
        outDegree$graph_name = graphName
        outDegree$name = "degree_out_centrality"
        inOutDegree <- cbind(rownames(inOutDegree), inOutDegree)
        colnames(inOutDegree) <- c("node_id", "value")
        inOutDegree$graph_name = graphName
        inOutDegree$name = "degree_in_out_centrality"

        # Insertion
        insertNodeAttribute(schema, inDegree)
        insertNodeAttribute(schema, outDegree)
        insertNodeAttribute(schema, inOutDegree)
    } else {
        print(sprintf("Impossible de calculer la centralité, graphe %s inexistant.", graphName))
    }
}


graphs <- getQuery(sprintf("SELECT name FROM \"%s\".graph WHERE graph_parent IS NULL", schema))

for (graph in graphs$name) {
    computeDegreeCentrality(graph)
}

computeHitsCentrality <- function(graphName) {
    print(sprintf("Calcul de la centralité HITS pour le graphe %s.", graphName))
    # Construction du graphe
    graph <- getGraph(schema, graphName)
    
    if (!is.null(graph)) {
        # Conservation uniquement de la plus grande composante connexe
        graph <- getMaxComposanteConnexe(graph)
        
        # Calcul des centralités
        # Authorities
        authorities <- as.data.frame(authority_score(graph, scale = TRUE, 
                                                     weights = as_data_frame(graph, what="edges")$weight)$vector)
        # Hubs
        hubs <- as.data.frame(hub_score(graph, scale = TRUE, 
                                       weights = as_data_frame(graph, what="edges")$weight)$vector)

        # Ajout du node_id en 1ere place du dataframe
        authorities <- cbind(rownames(authorities), authorities)
        colnames(authorities) <- c("node_id", "value")
        authorities$graph_name = graphName
        authorities$name = "hits_authority_centrality"
        hubs <- cbind(rownames(hubs), hubs)
        colnames(hubs) <- c("node_id", "value")
        hubs$graph_name = graphName
        hubs$name = "hits_hub_centrality"

        # Insertion
        insertNodeAttribute(schema, authorities)
        insertNodeAttribute(schema, hubs)
    } else {
        print(sprintf("Impossible de calculer la centralité, graphe %s inexistant.", graphName))
    }
}

graphs <- getQuery(sprintf("SELECT name FROM \"%s\".graph WHERE graph_parent IS NULL", schema))

for (graph in graphs$name) {
    computeHitsCentrality(graph)
}

computePageRankCentrality <- function(graphName) {
    print(sprintf("Calcul de la centralité page rank pour le graphe %s.", graphName))
    # Construction du graphe
    graph <- getGraph(schema, graphName)
    
    if (!is.null(graph)) {
        # Conservation uniquement de la plus grande composante connexe
        graph <- getMaxComposanteConnexe(graph)
        
        # Calcul de la centralité
        pageRank <- as.data.frame(page_rank(graph, vids = V(graph),
                                            weights = as_data_frame(graph, what="edges")$weight)$vector)

        # Ajout du node_id en 1ere place du dataframe
        pageRank <- cbind(rownames(pageRank), pageRank)
        colnames(pageRank) <- c("node_id", "value")
        pageRank$graph_name = graphName
        pageRank$name = "page_rank_centrality"

        # Insertion
        insertNodeAttribute(schema, pageRank)
    } else {
        print(sprintf("Impossible de calculer la centralité, graphe %s inexistant.", graphName))
    }
}

graphs <- getQuery(sprintf("SELECT name FROM \"%s\".graph WHERE graph_parent IS NULL", schema))

for (graph in graphs$name) {
    computePageRankCentrality(graph)
}


computeEdgeBetweennessCentrality <- function(graphName) {
    print(sprintf("Calcul de la centralité edge betweenness pour le graphe %s.", graphName))
    # Construction du graphe
    graph <- getGraph(schema, graphName)
    
    if (!is.null(graph)) {
        # Conservation uniquement de la plus grande composante connexe
        graph <- getMaxComposanteConnexe(graph)
        
        # Calcul de la centralité
        isDirected <- getQuery(sprintf("SELECT directed FROM \"%s\".graph WHERE name = '%s'", schema, graphName))
        edgeBetweenness <- as.data.frame(betweenness(graph, v = V(graph), directed = isDirected$directed[1],
                                            weights = as_data_frame(graph, what="edges")$weight))

        # Ajout du node_id en 1ere place du dataframe
        edgeBetweenness <- cbind(rownames(edgeBetweenness), edgeBetweenness)
        colnames(edgeBetweenness) <- c("node_id", "value")
        edgeBetweenness$graph_name = graphName
        edgeBetweenness$name = "edge_betweenness_centrality"

        # Insertion
        insertNodeAttribute(schema, edgeBetweenness)
    } else {
        print(sprintf("Impossible de calculer la centralité, graphe %s inexistant.", graphName))
    }
}


graphs <- getQuery(sprintf("SELECT name FROM \"%s\".graph WHERE graph_parent IS NULL", schema))

for (graph in graphs$name) {
    computeEdgeBetweennessCentrality(graph)
}

computeKCoreCentrality <- function(graphName) {
    print(sprintf("Calcul de la centralité k-core pour le graphe %s.", graphName))
    # Construction du graphe
    graph <- getGraph(schema, graphName)
    
    if (!is.null(graph)) {
        # Calcul des centralités
        inKCore <- as.data.frame(coreness(graph, mode = "in"))
        outKCore <- as.data.frame(coreness(graph, mode = "out"))
        inOutKCore <- as.data.frame(coreness(graph, mode = "all"))

        # Ajout du node_id en 1ere place du dataframe
        inKCore <- cbind(rownames(inKCore), inKCore)
        colnames(inKCore) <- c("node_id", "value")
        inKCore$graph_name = graphName
        inKCore$name = "k_core_in_centrality"
        outKCore <- cbind(rownames(outKCore), outKCore)
        colnames(outKCore) <- c("node_id", "value")
        outKCore$graph_name = graphName
        outKCore$name = "k_core_out_centrality"
        inOutKCore <- cbind(rownames(inOutKCore), inOutKCore)
        colnames(inOutKCore) <- c("node_id", "value")
        inOutKCore$graph_name = graphName
        inOutKCore$name = "k_core_in_out_centrality"

        # Insertion
        insertNodeAttribute(schema, inKCore)
        insertNodeAttribute(schema, outKCore)
        insertNodeAttribute(schema, inOutKCore)
    } else {
        print(sprintf("Impossible de calculer la centralité, graphe %s inexistant.", graphName))
    }
}


graphs <- getQuery(sprintf("SELECT name FROM \"%s\".graph WHERE graph_parent IS NULL", schema))

for (graph in graphs$name) {
    computeKCoreCentrality(graph)
}

computeAllCentralities <- function(graphName) {
    computeDegreeCentrality(graphName)
    computeHitsCentrality(graphName)
    computePageRankCentrality(graphName)
    computeEdgeBetweennessCentrality(graphName)
    computeKCoreCentrality(graphName)
}

isValidCommunity <- function(graph, community) {
    subGraph <- induced_subgraph(graph, community)
    sum(E(subGraph)$weight)
}

computeLouvainCommunity <- function(graphName) {
    print(sprintf("Calcul de communautés Louvain pour le graphe %s.", graphName))
    # Construction du graphe
    graph <- getGraph(schema, graphName)
    
    if (!is.null(graph)) {
        # Conservation uniquement de la plus grande composante connexe
        graph <- getMaxComposanteConnexe(graph)
        
        # Suppression des anciens sous-graphes existants
        sendQuery(sprintf("DELETE FROM \"%s\".graph 
            WHERE graph_parent = '%s'
                AND name ILIKE '%s_louvain_community_%%'", schema, graphName, graphName))
        
        # Calcul des communautés
        louvain <- cluster_louvain(as.undirected(graph))
        # Calcul de la modularité
        modularity <- modularity(as.undirected(graph), membership(louvain))
        print(sprintf("Modularité de Louvain : %s.", modularity))
        if (modularity > 0.5) {
            # Pour créer un dataframe vide mais avec les colonnes définies
            dfLouvain <- read.csv(text = "node_id,value")
            communityNumber <- 1
            saveModularity(schema, sprintf("%s_louvain_community", graphName), graphName, modularity)

            for (community in communities(louvain)) {
                # Récupération du sous-graphe de la communauté
                subGraph <- induced_subgraph(graph, community)
                # Vérification de la taille de la communauté
                if (vcount(subGraph) > sqrt(ecount(graph)/2)) {
                    # Ajout des noeuds dans le dataframe global
                    dfCommunity <- as.data.frame(community)
                    row.names(dfCommunity) <- dfCommunity$community
                    colnames(dfCommunity) <- c("id")

                    # Création du sous-graphe
                    subGraphName <- sprintf("%s_louvain_community_%d", graphName, communityNumber)
                    description <- sprintf("Sous-graphe représentant la communauté %d obtenue avec Louvain 
                                            à partir du graphe %s.", communityNumber, graphName)
                    notebookUrl <- "/user/jupyter/notebooks/vegan/CommunautesCentralites.ipynb#Louvain"
                    buildGraphFromGraphParentAndNodes(subGraphName, description, notebookUrl, graphName, dfCommunity, modularity)

                    # Ajout de la communauté courante dans le dataframe global
                    colnames(dfCommunity) <- c("node_id")
                    dfCommunity$value <- communityNumber
                    dfLouvain <- rbind(dfLouvain, dfCommunity)

                    # Calcul des centralités
                    computeAllCentralities(subGraphName)

                    # Incrémentation du numéro de la communauté
                    communityNumber <- communityNumber + 1
                }
            }
            
            if (nrow(dfLouvain) > 0) {
                # Insertion
                dfLouvain$graph_name <- graphName
                dfLouvain$name <- "louvain_community"
                insertNodeAttribute(schema, dfLouvain)
            }
        } else {
            saveModularity(schema, sprintf("%s_louvain_community", graphName), graphName, modularity)
            print("La modularité est < 0.5, pas de structure communautaire dans le graphe.")
        }
    } else {
        print(sprintf("Impossible de calculer les communautés, graphe %s inexistant.", graphName))
    }
}

graphs <- getQuery(sprintf("SELECT name FROM \"%s\".graph WHERE graph_parent IS NULL", schema))

for (graph in graphs$name) {
    computeLouvainCommunity(graph)
}


computeWalktrapCommunity <- function(graphName) {
    print(sprintf("Calcul de communautés Walktrap pour le graphe %s.", graphName))
    # Construction du graphe
    graph <- getGraph(schema, graphName)
    
    if (!is.null(graph)) {
        # Conservation uniquement de la plus grande composante connexe
        graph <- getMaxComposanteConnexe(graph)
        
        # Suppression des anciens sous-graphes existants
        sendQuery(sprintf("DELETE FROM \"%s\".graph 
            WHERE graph_parent = '%s'
                AND name ILIKE '%s_walktrap_community_%%'", schema, graphName, graphName))
        
        # Calcul des communautés
        walktrap <- cluster_walktrap(as.undirected(graph))
        # Calcul de la modularité
        modularity <- modularity(as.undirected(graph), membership(walktrap))
        print(sprintf("Modularité de Walktrap : %s.", modularity))
        if (modularity > 0.5) {
            # Pour créer un dataframe vide mais avec les colonnes définies
            dfWalktrap <- read.csv(text = "node_id,value")
            communityNumber <- 1
            saveModularity(schema, sprintf("%s_walktrap_community", graphName), name, modularity)
            
            for (community in communities(walktrap)) {
                # Récupération du sous-graphe de la communauté
                subGraph <- induced_subgraph(graph, community)
                # Vérification de la taille de la communauté
                if (vcount(subGraph) > sqrt(ecount(graph)/2)) {
                    # Ajout des noeuds dans le dataframe global
                    dfCommunity <- as.data.frame(community)
                    row.names(dfCommunity) <- dfCommunity$community
                    colnames(dfCommunity) <- c("id")

                    # Création du sous-graphe
                    subGraphName <- sprintf("%s_walktrap_community_%d", graphName, communityNumber)
                    description <- sprintf("Sous-graphe représentant la communauté %d obtenue avec Walktrap 
                                            à partir du graphe %s.", communityNumber, graphName)
                    notebookUrl <- "/user/jupyter/notebooks/vegan/CommunautesCentralites.ipynb#Walktrap"
                    buildGraphFromGraphParentAndNodes(subGraphName, description, notebookUrl, graphName, dfCommunity, modularity)

                    # Ajout de la communauté courante dans le dataframe global
                    colnames(dfCommunity) <- c("node_id")
                    dfCommunity$value <- communityNumber
                    dfWalktrap <- rbind(dfWalktrap, dfCommunity)

                    # Calcul des centralités
                    computeAllCentralities(subGraphName)

                    # Incrémentation du numéro de la communauté
                    communityNumber <- communityNumber + 1
                }
            }

            if (nrow(dfWalktrap) > 0) {
                # Insertion
                dfWalktrap$graph_name <- graphName
                dfWalktrap$name <- "walktrap_community"
                insertNodeAttribute(schema, dfWalktrap)
            }
        } else {
            saveModularity(schema, sprintf("%s_walktrap_community", graphName), name, modularity)
            print("La modularité est < 0.5, pas de structure communautaire dans le graphe.")
        }
    } else {
        print(sprintf("Impossible de calculer les communautés, graphe %s inexistant.", graphName))
    }
}

graphs <- getQuery(sprintf("SELECT name FROM \"%s\".graph WHERE graph_parent IS NULL", schema))

for (graph in graphs$name) {
    computeWalktrapCommunity(graph)
}


computeInfomapCommunity <- function(graphName) {
    print(sprintf("Calcul de communautés Infomap pour le graphe %s.", graphName))
    # Construction du graphe
    graph <- getGraph(schema, graphName)
    
    if (!is.null(graph)) {
        # Conservation uniquement de la plus grande composante connexe
        graph <- getMaxComposanteConnexe(graph)
        
        # Suppression des anciens sous-graphes existants
        sendQuery(sprintf("DELETE FROM \"%s\".graph 
            WHERE graph_parent = '%s'
                AND name ILIKE '%s_infomap_community_%%'", schema, graphName, graphName))
        
        # Calcul des communautés
        infomap <- cluster_infomap(as.undirected(graph))
        # Calcul de la modularité
        modularity <- modularity(as.undirected(graph), membership(infomap))
        print(sprintf("Modularité d'Infomap : %s.", modularity))
        if (modularity > 0.5) {
            # Pour créer un dataframe vide mais avec les colonnes définies
            dfInfomap <- read.csv(text = "node_id,value")
            communityNumber <- 1
            saveModularity(schema, sprintf("%s_infomap_community", graphName), name, modularity)

            for (community in communities(infomap)) {
                # Récupération du sous-graphe de la communauté
                subGraph <- induced_subgraph(graph, community)
                # Vérification de la taille de la communauté
                if (vcount(subGraph) > sqrt(ecount(graph)/2)) {
                    # Ajout des noeuds dans le dataframe global
                    dfCommunity <- as.data.frame(community)
                    row.names(dfCommunity) <- dfCommunity$community
                    colnames(dfCommunity) <- c("id")

                    # Création du sous-graphe
                    subGraphName <- sprintf("%s_infomap_community_%d", graphName, communityNumber)
                    description <- sprintf("Sous-graphe représentant la communauté %d obtenue avec Infomap 
                                            à partir du graphe %s.", communityNumber, graphName)
                    notebookUrl <- "/user/jupyter/notebooks/vegan/CommunautesCentralites.ipynb#Infomap"
                    buildGraphFromGraphParentAndNodes(subGraphName, description, notebookUrl, graphName, dfCommunity, modularity)

                    # Ajout de la communauté courante dans le dataframe global
                    colnames(dfCommunity) <- c("node_id")
                    dfCommunity$value <- communityNumber
                    dfInfomap <- rbind(dfInfomap, dfCommunity)

                    # Calcul des centralités
                    computeAllCentralities(subGraphName)

                    # Incrémentation du numéro de la communauté
                    communityNumber <- communityNumber + 1
                }
            }

            if (nrow(dfInfomap) > 0) {
                # Insertion
                dfInfomap$graph_name <- graphName
                dfInfomap$name <- "infomap_community"
                insertNodeAttribute(schema, dfInfomap)
            }
        } else {
            saveModularity(schema, sprintf("%s_infomap_community", graphName), name, modularity)
            print("La modularité est < 0.5, pas de structure communautaire dans le graphe.")
        }
    } else {
        print(sprintf("Impossible de calculer les communautés, graphe %s inexistant.", graphName))
    }
}


graphs <- getQuery(sprintf("SELECT name FROM \"%s\".graph WHERE graph_parent IS NULL", schema))

for (graph in graphs$name) {
    computeInfomapCommunity(graph)
}


computeEdgeBetweennessCommunity <- function(graphName) {
    print(sprintf("Calcul de communautés edge betweenness pour le graphe %s.", graphName))
    # Construction du graphe
    graph <- getGraph(schema, graphName)
    
    if (!is.null(graph)) {
        # Conservation uniquement de la plus grande composante connexe
        graph <- getMaxComposanteConnexe(graph)
        
        if (ecount(graph) < 5000) {
            # Suppression des anciens sous-graphes existants
            sendQuery(sprintf("DELETE FROM \"%s\".graph 
                WHERE graph_parent = '%s'
                    AND name ILIKE '%s_edge_betweenness_community_%%'", schema, graphName, graphName))

            # Calcul des communautés
            inverse <- function(n) {
                return(1/n)
            }
            distance <- sapply(E(graph)$weight, inverse)
            edgeBetweenness <- cluster_edge_betweenness(graph, weight = distance)
            # Calcul de la modularité
            modularity <- modularity(graph, membership(edgeBetweenness))
            print(sprintf("Modularité de edge betweenness : %s.", modularity))
            if (modularity > 0.5) {
                # Pour créer un dataframe vide mais avec les colonnes définies
                dfEdgeBetweenness <- read.csv(text = "node_id,value")
                communityNumber <- 1
                saveModularity(schema, sprintf("%s_edge_betweenness_community", graphName), name, modularity)

                for (community in communities(edgeBetweenness)) {
                    # Récupération du sous-graphe de la communauté
                    subGraph <- induced_subgraph(graph, community)
                    # Vérification de la taille de la communauté
                    if (vcount(subGraph) > sqrt(ecount(graph)/2)) {
                        # Ajout des noeuds dans le dataframe global
                        dfCommunity <- as.data.frame(community)
                        row.names(dfCommunity) <- dfCommunity$community
                        colnames(dfCommunity) <- c("id")

                        # Création du sous-graphe
                        subGraphName <- sprintf("%s_edge_betweenness_community_%d", graphName, communityNumber)
                        description <- sprintf("Sous-graphe représentant la communauté %d obtenue avec Edge Betweenness 
                                                à partir du graphe %s.", communityNumber, graphName)
                        notebookUrl <- "/user/jupyter/notebooks/vegan/CommunautesCentralites.ipynb#EdgeBetweenness"
                        buildGraphFromGraphParentAndNodes(subGraphName, description, notebookUrl, graphName, dfCommunity, modularity)

                        # Ajout de la communauté courante dans le dataframe global
                        colnames(dfCommunity) <- c("node_id")
                        dfCommunity$value <- communityNumber
                        dfEdgeBetweenness <- rbind(dfEdgeBetweenness, dfCommunity)

                        # Calcul des centralités
                        computeAllCentralities(subGraphName)

                        # Incrémentation du numéro de la communauté
                        communityNumber <- communityNumber + 1
                    }
                }

                if (nrow(dfEdgeBetweenness) > 0) {
                    # Insertion
                    dfEdgeBetweenness$graph_name <- graphName
                    dfEdgeBetweenness$name <- "edge_betweenness_community"
                    insertNodeAttribute(schema, dfEdgeBetweenness)
                }
            } else {
                saveModularity(schema, sprintf("%s_edge_betweenness_community", graphName), name, modularity)
                print("La modularité est < 0.5, pas de structure communautaire dans le graphe.")
            }
        } else {
            print(sprintf("Graphe %s trop volumineux pour exécuter la recherche de communautés avec edge betweenness.", 
                          graphName))
        }
    } else {
        print(sprintf("Impossible de calculer les communautés, graphe %s inexistant.", graphName))
    }
}

graphs <- getQuery(sprintf("SELECT name FROM \"%s\".graph WHERE graph_parent IS NULL", schema))

for (graph in graphs$name) {
    computeEdgeBetweennessCommunity(graph)
}

