import networkx as nx
from pyvis.network import Network
import psycopg2
from psycopg2.extras import RealDictCursor, Json
import json
import random
import sys

if len(sys.argv) != 7:
    sys.exit("il faut indiquer le schéma,la clef unique de l'étude et le type de graph")

schema = sys.argv[1]
graph_name = sys.argv[2]
community = sys.argv[3]
centrality = sys.argv[4]
max_rank = sys.argv[5]
show_interaction = sys.argv[6]

conn = psycopg2.connect("dbname=cocktail_pg user=cocktailuser password=cocktailuser host=localhost")
def create_vis_table():
    cur = conn.cursor()
    cur.execute(f"""
CREATE TABLE IF NOT EXISTS "{schema}".vis (graph_name VARCHAR NOT NULL, community VARCHAR NOT NULL, centrality VARCHAR NOT NULL, max_rank INTEGER NOT NULL, show_interaction BOOLEAN NOT NULL, nodes JSON NOT NULL, edges JSON NOT NULL)
    """)
    conn.commit()

listcolor = {}

def get_color_group(group):
    if group in listcolor:
        return listcolor[group]
    else:
        listcolor[group] = "rgb("+ str(random.randrange(0, 255)) + ',' + str(random.randrange(0, 255)) + ',' + str(random.randrange(0, 255)) + ')'
        return listcolor[group]

def generate_graph(query):
    # Préparation du graphe
    nx_graph = nx.Graph()
    g = Network("600px", "100%", notebook = False)

    cur = conn.cursor(cursor_factory=RealDictCursor)
    cur.execute(query)

    data = cur.fetchall()

    for e in data:
        src = e['source']
        dst = e['target']
        src_rank = int(e['source_rank'])
        dst_rank = int(e['target_rank'])
        src_w = 5 if src_rank > 10 else (65 - (src_rank * 5))
        dst_w = 5 if dst_rank > 10 else (65 - (dst_rank * 5))
        w = e['weight']
        src_color = int(e['source_community'])
        dst_color = int(e['target_community'])

        nx_graph.add_node(src, size = src_w, value = src_w, 
                          title = "Comunity: " + str(src_color) + "<br>Rank: " + str(src_rank) + "<br>" + src, 
                          group = src_color)
        nx_graph.add_node(dst, size = dst_w, value = dst_w, 
                          title = "Comunity: " + str(src_color) + "<br>Rank: " + str(dst_rank) + "<br>" + dst, 
                          group = dst_color)
        nx_graph.add_edge(src, dst, weight = w, color = get_color_group(src_color))

    layout = nx.spring_layout(nx_graph)
    g.from_nx(nx_graph)

    neighbor_map = g.get_adj_list()

    # add neighbor data to node hover data
    for node in g.nodes:
        node["title"] += " Neighbors:<br>" + "<br>".join(neighbor_map[node["id"]])
        node["x"] = layout[node["id"]][0] * 1000
        node["y"] = layout[node["id"]][1] * 1000
        node["font"] = {'size': node['size']}
        node["value"] = node['size']
        node["color"] = get_color_group(node['group'])

    create_vis_table()
    cur.execute(f"""
INSERT INTO "{schema}".vis (graph_name, community, centrality, max_rank, show_interaction, nodes, edges) VALUES(%s, %s, %s, %s, %s,%s, %s)
            """, 
            [graph_name, community, centrality, max_rank, show_interaction, Json(g.nodes), Json(g.edges)]
    )
    cur.execute(f"""
INSERT INTO "{schema}".status (status, graph_name, community, centrality, max_rank, show_interaction) VALUES('done', '{graph_name}', '{community}', '{centrality}', '{max_rank}', '{show_interaction}')
            """
    )

    conn.commit()

    return g

links_query = ""

if graph_name == "user_hashtag":
    #max_rank = 100
    links_query = f"""
        WITH node_rank AS (
            SELECT node_id, 
                graph_name, 
                name, 
                RANK () OVER (PARTITION BY graph_name, SUBSTRING(node_id FOR 1) ORDER BY value DESC, node_id) AS rank
            FROM "{schema}".node_attribute
            WHERE name = '{centrality}'
        ), node_complete AS (
            SELECT nc.graph_name AS graph_name, nr.node_id AS node_id, nr.rank AS rank, nc.value AS community
            FROM "{schema}".graph g
                INNER JOIN node_rank nr ON g.name = nr.graph_name
                INNER JOIN "{schema}".node_attribute nc ON g.graph_parent = nc.graph_name
                    AND nr.graph_name SIMILAR TO '{graph_name}_{community}_[1-9]+'
                    AND nr.node_id = nc.node_id
            WHERE nc.name = '{community}'
        )
        SELECT '@' || user1.screen_name AS source, 
            node2.node_id AS target, 
            node1.community AS source_community,
            node2.community AS target_community,
            node1.rank AS source_rank,
            node2.rank AS target_rank,
            weight
        FROM node_complete node1
            INNER JOIN "{schema}".link link ON node1.node_id = link.node_out 
                AND node1.graph_name = link.graph_name
            INNER JOIN node_complete node2 ON node2.node_id = link.node_in 
                AND node2.graph_name = link.graph_name
            INNER JOIN "{schema}".user user1 ON user1.id = SUBSTRING(link.node_out FROM 2)
        WHERE node1.graph_name = '{graph_name}'
            AND node1.rank <= {max_rank}
            AND node2.rank <= {max_rank}
            AND (node1.community = node2.community OR {show_interaction})
    """
elif graph_name == "hashtag_hashtag":
    #max_rank = 50
    links_query = f"""
        WITH node_rank AS (
            SELECT node_id, 
                graph_name, 
                name, 
                RANK () OVER (PARTITION BY graph_name ORDER BY value DESC, node_id) AS rank
            FROM "{schema}".node_attribute
            WHERE name = '{centrality}'
        ), node_complete AS (
            SELECT nc.graph_name AS graph_name, nr.node_id AS node_id, nr.rank AS rank, nc.value AS community
            FROM "{schema}".graph g
                INNER JOIN node_rank nr ON g.name = nr.graph_name
                INNER JOIN "{schema}".node_attribute nc ON g.graph_parent = nc.graph_name
                    AND nr.graph_name SIMILAR TO '{graph_name}_{community}_[1-9]+'
                    AND nr.node_id = nc.node_id
            WHERE nc.name = '{community}'
        )
        SELECT node1.node_id AS source, 
            node2.node_id AS target, 
            node1.community AS source_community,
            node2.community AS target_community,
            node1.rank AS source_rank,
            node2.rank AS target_rank,
            weight
        FROM node_complete node1
            INNER JOIN "{schema}".link link ON node1.node_id = link.node_out 
                AND node1.graph_name = link.graph_name
            INNER JOIN node_complete node2 ON node2.node_id = link.node_in 
                AND node2.graph_name = link.graph_name
        WHERE node1.graph_name = '{graph_name}'
            AND node1.rank <= {max_rank}
            AND node2.rank <= {max_rank}
            AND (node1.community = node2.community OR {show_interaction})
    """
else:
    #type par défaut user-user
    #max_rank = 200
    links_query = f"""
        WITH node_rank AS (
            SELECT node_id, 
                graph_name, 
                name, 
                RANK () OVER (PARTITION BY graph_name ORDER BY value DESC, node_id) AS rank
            FROM "{schema}".node_attribute
            WHERE name = '{centrality}'
        ), node_complete AS (
            SELECT nc.graph_name AS graph_name, nr.node_id AS node_id, nr.rank AS rank, nc.value AS community
            FROM "{schema}".graph g
                INNER JOIN node_rank nr ON g.name = nr.graph_name
                INNER JOIN "{schema}".node_attribute nc ON g.graph_parent = nc.graph_name
                    AND nr.graph_name SIMILAR TO '{graph_name}_{community}_[1-9]+'
                    AND nr.node_id = nc.node_id
            WHERE nc.name = '{community}'
        )
        SELECT user1.screen_name AS source, 
            user2.screen_name AS target, 
            node1.community AS source_community,
            node2.community AS target_community,
            node1.rank AS source_rank,
            node2.rank AS target_rank,
            weight
        FROM node_complete node1
            INNER JOIN "{schema}".link link ON node1.node_id = link.node_out 
                AND node1.graph_name = link.graph_name
            INNER JOIN node_complete node2 ON node2.node_id = link.node_in 
                AND node2.graph_name = link.graph_name
            INNER JOIN "{schema}".user user1 ON user1.id = link.node_out
            INNER JOIN "{schema}".user user2 ON user2.id = link.node_in
        WHERE node1.graph_name = '{graph_name}'
            AND node1.rank <= {max_rank}
            AND node2.rank <= {max_rank}
            AND (node1.community = node2.community OR {show_interaction})
    """

g = generate_graph(links_query)
print(json.dumps({ "nodes": g.nodes, "edges": g.edges }))
