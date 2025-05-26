library('RPostgreSQL')

getQuery <- function(query) {
	drv <- dbDriver("PostgreSQL")
	con <- dbConnect(drv, 
		dbname="cocktail_pg", 
		user="cocktailuser", 
		password="cocktailuser", 
		port=5432, 
		host="localhost")
	on.exit(dbDisconnect(con))
	dbGetQuery(con, query)
}

sendQuery <- function(query, ...) {
	drv <- dbDriver("PostgreSQL")
	con <- dbConnect(drv, 
		dbname="cocktail_pg", 
		user="cocktailuser", 
		password="cocktailuser", 
		port=5432, 
		host="localhost")
	on.exit(dbDisconnect(con))
	dbSendQuery(con, query, ...)
}

writeTable <- function(name, value, ...) {
	drv <- dbDriver("PostgreSQL")
	con <- dbConnect(drv, 
		dbname="cocktail_pg", 
		user="cocktailuser", 
		password="cocktailuser", 
		port=5432, 
		host="localhost")
	on.exit(dbDisconnect(con))
	dbWriteTable(con, name, value, ...)
}

buildGraph <- function(schema, name, description, linksQuery, directed = TRUE) {
   # Création du graphe
    sendQuery(sprintf("INSERT INTO \"%s\".graph (name, description, directed, notebook_url) VALUES (
        '%s',
        '%s',
        %d::BOOLEAN,
        'pathnotebook/CommunautesCentralites.ipynb#%s'
    )", schema, name, description, directed, name))
    
    # Création des noeuds
    sendQuery(sprintf("INSERT INTO \"%s\".node (id, graph_name) 
        SELECT DISTINCT node AS id, '%s' AS graph_name FROM (
            SELECT DISTINCT node_out AS node
            FROM (%s) AS t
                UNION
            SELECT DISTINCT node_in AS node
            FROM (%s) AS t
    ) AS t", schema, name, linksQuery, linksQuery))

    # Création des liens
    sendQuery(sprintf("INSERT INTO \"%s\".link (node_out, node_in, graph_name, weight) 
        SELECT node_out, node_in, '%s' AS graph_name, weight
        FROM (%s) AS t", schema, name, linksQuery))
}

saveModularity <- function(schema, name, parent, modularity) {
    sendQuery(sprintf("INSERT INTO \"%s\".graph (name, graph_parent, modularity) VALUES (
            '%s',
            '%s',
            %f
        )
        ON CONFLICT (name) DO UPDATE
        SET modularity = EXCLUDED.modularity;
    ", schema, name, parent, modularity))
}

insertNodeAttribute <- function(schema, df) {
    sendQuery(sprintf("DELETE FROM \"%s\".node_attribute WHERE graph_name = '%s' AND name = '%s'", schema, df$graph_name[1], df$name[1]))
    writeTable(name = c(schema, 'node_attribute'), value = df, overwrite = FALSE, append = TRUE, row.names = FALSE)
}

getGraph <- function(schema, graphName) {
    nodesQuery <- sprintf("SELECT id FROM \"%s\".node WHERE graph_name = '%s'", schema, graphName)
    linksQuery <- sprintf("SELECT node_out AS \"from\", node_in AS \"to\", weight
                        FROM \"%s\".link WHERE graph_name = '%s'", schema, graphName)
    isDirectedQuery <- sprintf("SELECT directed FROM \"%s\".graph WHERE name = '%s'", schema, graphName)
       countQuery <- sprintf("SELECT count(*) as  \"count\" FROM \"%s\".link WHERE graph_name = '%s'", schema, graphName)
    nodes <- getQuery(nodesQuery)
    links <- getQuery(linksQuery)
    count <- getQuery(countQuery)
    isDirected <- getQuery(isDirectedQuery)$directed[1]
    if (count > 0) {
        graph <- graph_from_data_frame(links, directed = isDirected, vertices = nodes$id)
    } else {
        graph <- NULL
    }
    return(graph)
}